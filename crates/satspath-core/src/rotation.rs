use serde::{Deserialize, Serialize};

use crate::crypto::{verify_message_signature, sign_message};
use crate::{SignedPaymentProfile, Result};

/// A proof of key rotation.
/// If present, the `PaymentProfile::identity_pubkey` is the new key, and this object
/// proves that the rotation was authorized by the old key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotation {
    /// The previous identity public key (hex).
    pub previous_pubkey: String,
    /// The new identity public key (hex) - must match the profile's identity_pubkey.
    pub new_pubkey: String,
    /// Signature of the new pubkey created by the previous secret key.
    pub authorization_signature: String,
    /// The timestamp when the rotation occurred.
    pub rotated_at: i64,
}

impl KeyRotation {
    /// Create a new KeyRotation object, signing the new pubkey with the old secret key.
    pub fn create(
        previous_pubkey_hex: String,
        old_secret_key: &secp256k1::SecretKey,
        new_pubkey_hex: String,
    ) -> Result<Self> {
        let rotated_at = chrono::Utc::now().timestamp();
        let message = format!("RotateSatsPathKey:{}->{}@{}", previous_pubkey_hex, new_pubkey_hex, rotated_at);
        let signature = sign_message(&message, old_secret_key);
        Ok(Self {
            previous_pubkey: previous_pubkey_hex,
            new_pubkey: new_pubkey_hex,
            authorization_signature: signature,
            rotated_at,
        })
    }

    /// Verify the rotation authorization signature.
    pub fn verify(&self) -> Result<bool> {
        let message = format!("RotateSatsPathKey:{}->{}@{}", self.previous_pubkey, self.new_pubkey, self.rotated_at);
        verify_message_signature(&message, &self.authorization_signature, &self.previous_pubkey)
    }
}

/// Apply a key rotation to a signed payment profile.
/// The profile must have a valid KeyRotation, and the new pubkey must match the rotation's new_pubkey.
pub fn apply_key_rotation(profile: &SignedPaymentProfile, new_pubkey_hex: &str) -> Result<SignedPaymentProfile> {
    let rotation = profile.profile.rotation.as_ref()
        .ok_or_else(|| crate::errors::SatsPathError::CryptoError("no key rotation in profile".into()))?;
    
    if rotation.new_pubkey != new_pubkey_hex {
        return Err(crate::errors::SatsPathError::CryptoError("new pubkey doesn't match rotation".into()));
    }
    
    if !rotation.verify()? {
        return Err(crate::errors::SatsPathError::InvalidSignature);
    }
    
    // Create new profile with updated identity pubkey
    let mut new_profile = profile.profile.clone();
    new_profile.identity_pubkey = new_pubkey_hex.to_string();
    // Clear rotation since it's been applied
    new_profile.rotation = None;
    
    // The new profile needs to be signed with the NEW secret key
    // For now, we return the profile without signature (caller must re-sign)
    Ok(SignedPaymentProfile {
        profile: new_profile,
        signature: String::new(),
            hybrid_signature: None, // Placeholder - must be re-signed
    })
}

/// Get the effective identity pubkey, considering key rotation.
/// If a valid key rotation is present, returns the new pubkey.
/// Otherwise returns the profile's identity_pubkey.
pub fn get_effective_identity_pubkey(profile: &SignedPaymentProfile) -> Result<String> {
    if let Some(rotation) = &profile.profile.rotation {
        if rotation.verify()? {
            return Ok(rotation.new_pubkey.clone());
        }
    }
    Ok(profile.profile.identity_pubkey.clone())
}

/// Check if a key rotation is valid.
pub fn is_rotation_valid(profile: &SignedPaymentProfile) -> Result<bool> {
    if let Some(rotation) = &profile.profile.rotation {
        rotation.verify()
    } else {
        Ok(false)
    }
}

/// Rotate the identity key of a signed payment profile.
/// This creates a new KeyRotation and updates the profile.
/// Returns the new profile with the rotation applied (but not yet signed).
pub fn rotate_identity_key(
    profile: &SignedPaymentProfile,
    new_secret_key: &secp256k1::SecretKey,
) -> Result<SignedPaymentProfile> {
    let secp = secp256k1::Secp256k1::new();
    let new_public_key = secp256k1::PublicKey::from_secret_key(&secp, new_secret_key);
    let new_pubkey_hex = hex::encode(new_public_key.serialize());
    
    let old_pubkey_hex = profile.profile.identity_pubkey.clone();
    
    let rotation = KeyRotation::create(old_pubkey_hex, new_secret_key, new_pubkey_hex.clone())?;
    
    let mut new_profile = profile.profile.clone();
    new_profile.identity_pubkey = new_pubkey_hex;
    new_profile.rotation = Some(rotation);
    // Clear signature since it needs to be re-signed with the new key
    // The caller must re-sign with the new secret key
    Ok(SignedPaymentProfile {
        profile: new_profile,
        signature: String::new(),
            hybrid_signature: None,
    })
}

/// Verify a key rotation between old and new profiles.
pub fn verify_key_rotation(old_profile: &SignedPaymentProfile, new_profile: &SignedPaymentProfile) -> Result<bool> {
    // The new profile should have a rotation pointing from old pubkey to new pubkey
    if let Some(rotation) = &new_profile.profile.rotation {
        if rotation.previous_pubkey != old_profile.profile.identity_pubkey {
            return Ok(false);
        }
        if rotation.new_pubkey != new_profile.profile.identity_pubkey {
            return Ok(false);
        }
        rotation.verify()
    } else {
        Ok(false)
    }
}