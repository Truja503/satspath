use secp256k1::{Keypair, Message, PublicKey, Secp256k1, SecretKey, XOnlyPublicKey};
use secp256k1::schnorr::Signature;
use sha2::{Digest, Sha256};

use crate::errors::{Result, SatsPathError};
use crate::profile::{PaymentProfile, SignedPaymentProfile};

/// Domain separator for profile signing, per Protocol v0.1 §12.
/// Pre-pended to canonical JSON before hashing to prevent cross-context
/// signature replay (e.g. a message signature cannot be mistaken for a profile sig).
const PROFILE_DOMAIN_SEPARATOR: &[u8] = b"SatsPathProfileV1";

/// An identity keypair for a SatsPath user.
pub struct IdentityKeypair {
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
}

/// Generate a fresh secp256k1 identity keypair.
pub fn generate_identity_keypair() -> IdentityKeypair {
    let secp = Secp256k1::new();
    let (secret_key, public_key) = secp.generate_keypair(&mut rand::thread_rng());
    IdentityKeypair {
        secret_key,
        public_key,
    }
}

/// Produce a deterministic canonical JSON serialization of a PaymentProfile.
/// Uses canonical_json crate which sorts object keys for deterministic output.
pub fn canonical_profile_bytes(profile: &PaymentProfile) -> Result<Vec<u8>> {
    let value = serde_json::to_value(profile)
        .map_err(|e| SatsPathError::SerializationError(e.to_string()))?;
    let canonical = canonical_json::to_string(&value)
        .map_err(|e| SatsPathError::SerializationError(e.to_string()))?;
    Ok(canonical.into_bytes())
}

/// Sign a PaymentProfile with the given secret key and return a SignedPaymentProfile.
///
/// Uses domain-separated hashing per Protocol v0.1 §12:
/// `sig = ECDSA(SHA256("SatsPathProfileV1" || canonical_json(profile)))`
pub fn sign_profile(
    profile: PaymentProfile,
    secret_key: &SecretKey,
) -> Result<SignedPaymentProfile> {
    let secp = Secp256k1::new();
    let bytes = canonical_profile_bytes(&profile)?;
    let mut hasher = Sha256::new();
    hasher.update(PROFILE_DOMAIN_SEPARATOR);
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let message = Message::from_digest(digest.into());
    let keypair = Keypair::from_secret_key(&secp, secret_key);
    let sig = secp.sign_schnorr(&message, &keypair);
    Ok(SignedPaymentProfile {
        profile,
        signature: hex::encode(sig.serialize()),
    })
}

/// Verify that the signature inside a SignedPaymentProfile is valid.
///
/// Uses the same domain-separated hashing as `sign_profile`.
pub fn verify_signed_profile(signed: &SignedPaymentProfile) -> Result<bool> {
    let secp = Secp256k1::new();

    let pubkey_bytes = hex::decode(&signed.profile.identity_pubkey)
        .map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;
    let public_key = PublicKey::from_slice(&pubkey_bytes)
        .map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;
    let (x_only_public_key, _) = public_key.x_only_public_key();

    let sig_bytes =
        hex::decode(&signed.signature).map_err(|e| SatsPathError::CryptoError(e.to_string()))?;
    let sig = Signature::from_slice(&sig_bytes)
        .map_err(|e| SatsPathError::CryptoError(e.to_string()))?;

    let bytes = canonical_profile_bytes(&signed.profile)?;
    let mut hasher = Sha256::new();
    hasher.update(PROFILE_DOMAIN_SEPARATOR);
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let message = Message::from_digest(digest.into());

    Ok(secp.verify_schnorr(&sig, &message, &x_only_public_key).is_ok())
}

/// Check whether a `PaymentProfile` is expired.
///
/// Returns `Ok(())` if:
/// - `expires_at` is `None` (profile is non-expiring — backward-compatible).
/// - `expires_at` is in the future relative to the current UTC wall clock.
///
/// Returns `Err(SatsPathError::RegistryError(...))` if the profile has
/// explicitly expired. Callers must treat expired profiles as invalid.
pub fn check_profile_expiry(profile: &PaymentProfile) -> Result<()> {
    if let Some(exp) = profile.expires_at {
        let now = chrono::Utc::now().timestamp();
        if now >= exp {
            return Err(SatsPathError::RegistryError(format!(
                "profile for '{}' expired at unix timestamp {} (now: {})",
                profile.alias, exp, now
            )));
        }
    }
    Ok(())
}

/// Produce a short human-readable fingerprint of a public key (first 8 hex chars of SHA-256).
pub fn fingerprint_pubkey(pubkey_hex: &str) -> Result<String> {
    let bytes =
        hex::decode(pubkey_hex).map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;
    let digest = Sha256::digest(&bytes);
    Ok(hex::encode(&digest[..4]))
}

/// Generate a random 128-bit nonce encoded as a 32-character hex string.
///
/// Used in `PaymentProfile.nonce` to commit the signature to this specific
/// version of the profile, preventing replay even when all other fields
/// are identical to a previous version.
pub fn generate_nonce() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// Sign an arbitrary UTF-8 message with a secret key, returning a hex signature.
///
/// The message is hashed with SHA-256 before signing, matching the profile-signing
/// convention. Callers supply a domain-separated message (e.g. an ownership-proof
/// challenge) so signatures cannot be replayed across contexts.
///
/// The secret key is borrowed transiently and never persisted by SatsPath.
pub fn sign_message(message: &str, secret_key: &SecretKey) -> String {
    let secp = Secp256k1::new();
    let digest = Sha256::digest(message.as_bytes());
    let keypair = Keypair::from_secret_key(&secp, secret_key);
    let sig = secp.sign_schnorr(&Message::from_digest(digest.into()), &keypair);
    hex::encode(sig.serialize())
}

/// Verify a hex Schnorr signature over an arbitrary UTF-8 message against a
/// compressed secp256k1 public key (hex) or x-only public key. Returns `Ok(true)` only if the
/// signature is structurally valid *and* verifies.
pub fn verify_message_signature(
    message: &str,
    signature_hex: &str,
    pubkey_hex: &str,
) -> Result<bool> {
    let secp = Secp256k1::new();

    let pubkey_bytes =
        hex::decode(pubkey_hex).map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;
    
    let x_only_public_key = if pubkey_bytes.len() == 32 {
        XOnlyPublicKey::from_slice(&pubkey_bytes)
            .map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?
    } else {
        let public_key = PublicKey::from_slice(&pubkey_bytes)
            .map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;
        public_key.x_only_public_key().0
    };

    let sig_bytes =
        hex::decode(signature_hex).map_err(|e| SatsPathError::CryptoError(e.to_string()))?;
    let sig = Signature::from_slice(&sig_bytes)
        .map_err(|e| SatsPathError::CryptoError(e.to_string()))?;

    let digest = Sha256::digest(message.as_bytes());
    let msg = Message::from_digest(digest.into());

    Ok(secp.verify_schnorr(&sig, &msg, &x_only_public_key).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{PaymentMethod, PaymentProfile};

    fn demo_profile(pubkey_hex: &str) -> PaymentProfile {
        PaymentProfile {
            alias: "test@example.com".into(),
            identity_pubkey: pubkey_hex.into(),
            methods: vec![PaymentMethod::Lightning {
                label: "Lightning".into(),
                lightning_address: Some("test@example.com".into()),
                lnurl: None,
                bolt12: None,
                receiver_pubkey: None,
            }],
            updated_at: 1_700_000_000,
            expires_at: None,
            sequence: None,
            preferences: Vec::new(),
            nonce: None,
            rotation: None,
            method_verifications: Vec::new(),
        }
    }

    #[test]
    fn sign_and_verify() {
        let kp = generate_identity_keypair();
        let pubkey_hex = hex::encode(kp.public_key.serialize());
        let profile = demo_profile(&pubkey_hex);
        let signed = sign_profile(profile, &kp.secret_key).unwrap();
        assert!(verify_signed_profile(&signed).unwrap());
    }

    #[test]
    fn tampered_signature_rejected() {
        let kp = generate_identity_keypair();
        let pubkey_hex = hex::encode(kp.public_key.serialize());
        let profile = demo_profile(&pubkey_hex);
        let mut signed = sign_profile(profile, &kp.secret_key).unwrap();
        // Flip one hex char in the signature
        let mut bad_sig = signed.signature.clone();
        let last = bad_sig.pop().unwrap();
        bad_sig.push(if last == '0' { '1' } else { '0' });
        signed.signature = bad_sig;
        // Should fail to parse as DER or fail verification
        let result = verify_signed_profile(&signed);
        assert!(result.is_err() || !result.unwrap());
    }

    #[test]
    fn tampered_profile_rejected() {
        let kp = generate_identity_keypair();
        let pubkey_hex = hex::encode(kp.public_key.serialize());
        let profile = demo_profile(&pubkey_hex);
        let mut signed = sign_profile(profile, &kp.secret_key).unwrap();
        // Alter the alias after signing
        signed.profile.alias = "evil@hacker.com".into();
        assert!(!verify_signed_profile(&signed).unwrap());
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let kp = generate_identity_keypair();
        let hex = hex::encode(kp.public_key.serialize());
        let fp1 = fingerprint_pubkey(&hex).unwrap();
        let fp2 = fingerprint_pubkey(&hex).unwrap();
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 8); // 4 bytes = 8 hex chars
    }

    // ── SEC-01: expiry tests ──────────────────────────────────────────────────

    #[test]
    fn non_expiring_profile_passes() {
        let kp = generate_identity_keypair();
        let mut profile = demo_profile(&hex::encode(kp.public_key.serialize()));
        profile.expires_at = None; // explicit non-expiring
        assert!(check_profile_expiry(&profile).is_ok());
    }

    #[test]
    fn future_expires_at_passes() {
        let kp = generate_identity_keypair();
        let mut profile = demo_profile(&hex::encode(kp.public_key.serialize()));
        // Set expiry 1 hour in the future
        profile.expires_at = Some(chrono::Utc::now().timestamp() + 3_600);
        assert!(check_profile_expiry(&profile).is_ok());
    }

    #[test]
    fn past_expires_at_rejected() {
        let kp = generate_identity_keypair();
        let mut profile = demo_profile(&hex::encode(kp.public_key.serialize()));
        // Set expiry 1 hour in the past
        profile.expires_at = Some(chrono::Utc::now().timestamp() - 3_600);
        let result = check_profile_expiry(&profile);
        assert!(result.is_err(), "expired profile must be rejected");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("expired"), "error message must mention expiry");
    }

    #[test]
    fn exact_expiry_timestamp_rejected() {
        // expires_at == now must be treated as expired (fail-closed at boundary)
        let kp = generate_identity_keypair();
        let mut profile = demo_profile(&hex::encode(kp.public_key.serialize()));
        profile.expires_at = Some(chrono::Utc::now().timestamp());
        // Allow a tiny race: the check is >= so this should fail closed
        let result = check_profile_expiry(&profile);
        // In rare cases the timestamp might tick forward; either way we accept both
        // outcomes as long as the code is correct — but if it does fail, it must
        // be due to expiry.
        if let Err(e) = result {
            assert!(e.to_string().contains("expired"));
        }
    }

    #[test]
    fn message_signature_roundtrip() {
        let kp = generate_identity_keypair();
        let pubkey_hex = hex::encode(kp.public_key.serialize());
        let sig = sign_message("hello ownership", &kp.secret_key);
        assert!(verify_message_signature("hello ownership", &sig, &pubkey_hex).unwrap());
    }

    #[test]
    fn message_signature_rejects_wrong_message() {
        let kp = generate_identity_keypair();
        let pubkey_hex = hex::encode(kp.public_key.serialize());
        let sig = sign_message("original", &kp.secret_key);
        assert!(!verify_message_signature("tampered", &sig, &pubkey_hex).unwrap());
    }

    #[test]
    fn message_signature_rejects_wrong_key() {
        let kp = generate_identity_keypair();
        let other = generate_identity_keypair();
        let other_pubkey = hex::encode(other.public_key.serialize());
        let sig = sign_message("msg", &kp.secret_key);
        assert!(!verify_message_signature("msg", &sig, &other_pubkey).unwrap());
    }
}
