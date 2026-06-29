//! Local identity-key store.
//!
//! Holds the **protocol identity key** — the secp256k1 key that signs a user's
//! public payment profile. It is *not* a wallet seed, spending key, xprv,
//! mnemonic, macaroon, or API secret, and it never enters a profile, a QR
//! payload, or the network.
//!
//! Keys live under `.satspath/identity/<identity_pubkey>.key` (hex). `.satspath/`
//! and `*.key` are both gitignored, and the file is created owner-only on Unix.
//! Persisting it is what lets a user attach ownership proofs to — and otherwise
//! update — their own profile later, since doing so re-signs the profile.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use secp256k1::{PublicKey, Secp256k1, SecretKey};

const IDENTITY_SUBDIR: &str = "identity";

fn identity_dir(base: &Path) -> PathBuf {
    base.join(IDENTITY_SUBDIR)
}

/// Path of the identity key file for a given identity pubkey (hex).
pub fn identity_key_path(base: &Path, identity_pubkey_hex: &str) -> PathBuf {
    identity_dir(base).join(format!("{identity_pubkey_hex}.key"))
}

fn pubkey_hex_of(secret_key: &SecretKey) -> String {
    let secp = Secp256k1::new();
    hex::encode(PublicKey::from_secret_key(&secp, secret_key).serialize())
}

/// Persist the protocol identity secret key. Returns the path written.
pub fn save_identity_key(base: &Path, secret_key: &SecretKey) -> Result<PathBuf> {
    let pubkey_hex = pubkey_hex_of(secret_key);
    let dir = identity_dir(base);
    fs::create_dir_all(&dir).context("creating identity keystore directory")?;
    let path = identity_key_path(base, &pubkey_hex);
    fs::write(&path, hex::encode(secret_key.secret_bytes())).context("writing identity key")?;
    set_owner_only(&path)?;
    Ok(path)
}

/// Load the identity secret key for a given identity pubkey (hex).
pub fn load_identity_key(base: &Path, identity_pubkey_hex: &str) -> Result<SecretKey> {
    let path = identity_key_path(base, identity_pubkey_hex);
    if !path.exists() {
        anyhow::bail!(
            "identity key not found at {}.\n\
             This profile cannot be modified on this machine — it was registered \
             elsewhere, or before identity keys were saved.",
            path.display()
        );
    }
    let hex_str = fs::read_to_string(&path).context("reading identity key")?;
    let bytes = hex::decode(hex_str.trim()).context("decoding identity key hex")?;
    let secret_key = SecretKey::from_slice(&bytes).context("parsing identity key")?;
    // The file is named by pubkey; make sure the contents actually match it.
    if pubkey_hex_of(&secret_key) != identity_pubkey_hex {
        anyhow::bail!("identity key file does not match the requested identity pubkey");
    }
    Ok(secret_key)
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use satspath_core::crypto::generate_identity_keypair;

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let kp = generate_identity_keypair();
        let pubkey_hex = hex::encode(kp.public_key.serialize());

        let path = save_identity_key(dir.path(), &kp.secret_key).unwrap();
        assert!(path.exists());

        let loaded = load_identity_key(dir.path(), &pubkey_hex).unwrap();
        assert_eq!(loaded.secret_bytes(), kp.secret_key.secret_bytes());
    }

    #[test]
    fn load_missing_key_errors() {
        let dir = tempfile::tempdir().unwrap();
        let kp = generate_identity_keypair();
        let pubkey_hex = hex::encode(kp.public_key.serialize());
        assert!(load_identity_key(dir.path(), &pubkey_hex).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn saved_key_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let kp = generate_identity_keypair();
        let path = save_identity_key(dir.path(), &kp.secret_key).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
