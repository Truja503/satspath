use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::{Result, SatsPathError};
use crate::privacy::{canonical_identifier, identifier_hash};
use crate::profile::SignedPaymentProfile;

const REGISTRY_FILE: &str = "registry.json";

#[derive(Debug, Serialize, Deserialize, Default)]
struct RegistryData {
    profiles: HashMap<String, SignedPaymentProfile>,
}

/// Local file-backed registry of signed payment profiles.
///
/// In production this would be replaced with BIP-353, Nostr, or a
/// decentralized registry. For the hackathon prototype it persists to
/// `.satspath/registry.json` on disk.
pub struct Registry {
    path: PathBuf,
    data: RegistryData,
}

impl Registry {
    /// Open (or create) the registry at `dir/registry.json`.
    pub fn open(dir: &Path) -> Result<Self> {
        let path = dir.join(REGISTRY_FILE);
        let data = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            serde_json::from_str(&raw)?
        } else {
            RegistryData::default()
        };
        Ok(Registry { path, data })
    }

    fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.data)
            .map_err(|e| SatsPathError::SerializationError(e.to_string()))?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, json)?;
        Ok(())
    }

    /// Register a signed profile. Fails if the alias is already taken.
    pub fn register_profile(&mut self, signed: SignedPaymentProfile) -> Result<()> {
        let alias = canonical_identifier(&signed.profile.alias);
        let key = identifier_hash(&alias);
        if self.data.profiles.contains_key(&key) || self.data.profiles.contains_key(&alias) {
            return Err(SatsPathError::AliasAlreadyRegistered(alias));
        }
        self.data.profiles.insert(key, signed);
        self.save()
    }

    /// Update (overwrite) an existing profile entry.
    pub fn update_profile(&mut self, signed: SignedPaymentProfile) -> Result<()> {
        let alias = canonical_identifier(&signed.profile.alias);
        let key = identifier_hash(&alias);
        
        // SEC-03: Downgrade Attack Mitigation
        // Ensure we do not overwrite a newer profile with an older one.
        if let Some(existing) = self.data.profiles.get(&key).or_else(|| self.data.profiles.get(&alias)) {
            if signed.profile.updated_at < existing.profile.updated_at {
                return Err(SatsPathError::RegistryError(format!(
                    "Update rejected: incoming profile is older (updated_at: {}) than existing profile (updated_at: {})",
                    signed.profile.updated_at, existing.profile.updated_at
                )));
            }
        }
        
        self.data.profiles.insert(key, signed);
        self.save()
    }

    /// Resolve an alias to its signed profile.
    pub fn resolve_alias(&self, alias: &str) -> Result<&SignedPaymentProfile> {
        let canonical = canonical_identifier(alias);
        let key = identifier_hash(&canonical);
        self.data
            .profiles
            .get(&key)
            .or_else(|| self.data.profiles.get(&canonical))
            .ok_or(SatsPathError::AliasNotFound(canonical))
    }

    /// Check whether an alias is already registered.
    pub fn is_registered(&self, alias: &str) -> bool {
        let canonical = canonical_identifier(alias);
        self.data
            .profiles
            .contains_key(&identifier_hash(&canonical))
            || self.data.profiles.contains_key(&canonical)
    }

    /// Return all registered aliases.
    pub fn all_aliases(&self) -> Vec<&str> {
        self.data.profiles.keys().map(String::as_str).collect()
    }
}

use crate::resolver::ProfileResolver;
use async_trait::async_trait;

#[async_trait]
impl ProfileResolver for Registry {
    async fn resolve_alias(&self, alias: &str) -> Result<SignedPaymentProfile> {
        // We clone to return an owned value, as ProfileResolver returns owned data
        let signed = self.resolve_alias(alias).cloned()?;
        // SEC-01: enforce profile expiry before returning.
        crate::crypto::check_profile_expiry(&signed.profile)?;
        Ok(signed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{generate_identity_keypair, sign_profile};
    use crate::profile::{PaymentMethod, PaymentProfile};

    fn make_signed(alias: &str) -> SignedPaymentProfile {
        let kp = generate_identity_keypair();
        let pubkey_hex = hex::encode(kp.public_key.serialize());
        let profile = PaymentProfile {
            alias: alias.to_string(),
            identity_pubkey: pubkey_hex,
            methods: vec![PaymentMethod::Lightning {
                label: "LN".into(),
                lightning_address: Some(alias.to_string()),
                lnurl: None,
                bolt12: None,
                receiver_pubkey: None,
            }],
            updated_at: 1_700_000_000,
            expires_at: None,
            method_verifications: Vec::new(),
        };
        sign_profile(profile, &kp.secret_key).unwrap()
    }

    #[test]
    fn register_and_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let mut reg = Registry::open(dir.path()).unwrap();
        let signed = make_signed("alice@example.com");
        reg.register_profile(signed).unwrap();
        let resolved = reg.resolve_alias("alice@example.com").unwrap();
        assert_eq!(resolved.profile.alias, "alice@example.com");
    }

    #[test]
    fn duplicate_registration_fails() {
        let dir = tempfile::tempdir().unwrap();
        let mut reg = Registry::open(dir.path()).unwrap();
        reg.register_profile(make_signed("bob@example.com"))
            .unwrap();
        let err = reg
            .register_profile(make_signed("bob@example.com"))
            .unwrap_err();
        assert!(matches!(err, SatsPathError::AliasAlreadyRegistered(_)));
    }

    #[test]
    fn missing_alias_fails() {
        let dir = tempfile::tempdir().unwrap();
        let reg = Registry::open(dir.path()).unwrap();
        let err = reg.resolve_alias("ghost@example.com").unwrap_err();
        assert!(matches!(err, SatsPathError::AliasNotFound(_)));
    }

    #[test]
    fn is_registered_check() {
        let dir = tempfile::tempdir().unwrap();
        let mut reg = Registry::open(dir.path()).unwrap();
        assert!(!reg.is_registered("carol@example.com"));
        reg.register_profile(make_signed("carol@example.com"))
            .unwrap();
        assert!(reg.is_registered("carol@example.com"));
    }

    #[test]
    fn registry_persists_across_opens() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut reg = Registry::open(dir.path()).unwrap();
            reg.register_profile(make_signed("persist@example.com"))
                .unwrap();
        }
        let reg2 = Registry::open(dir.path()).unwrap();
        assert!(reg2.is_registered("persist@example.com"));
    }
}
