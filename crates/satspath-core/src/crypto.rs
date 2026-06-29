use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};

use crate::errors::{Result, SatsPathError};
use crate::profile::{PaymentProfile, SignedPaymentProfile};

/// An identity keypair for a SatsPath user.
pub struct IdentityKeypair {
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
}

/// Generate a fresh secp256k1 identity keypair.
pub fn generate_identity_keypair() -> IdentityKeypair {
    let secp = Secp256k1::new();
    let (secret_key, public_key) = secp.generate_keypair(&mut rand::thread_rng());
    IdentityKeypair { secret_key, public_key }
}

/// Produce a deterministic canonical JSON serialization of a PaymentProfile.
/// Fields are serialized in a fixed key order so signatures are reproducible.
pub fn canonical_profile_bytes(profile: &PaymentProfile) -> Result<Vec<u8>> {
    // serde_json preserves insertion order for structs (via derived Serialize),
    // which is deterministic across runs on the same binary version.
    let json = serde_json::to_string(profile)
        .map_err(|e| SatsPathError::SerializationError(e.to_string()))?;
    Ok(json.into_bytes())
}

/// Sign a PaymentProfile with the given secret key and return a SignedPaymentProfile.
pub fn sign_profile(profile: PaymentProfile, secret_key: &SecretKey) -> Result<SignedPaymentProfile> {
    let secp = Secp256k1::new();
    let bytes = canonical_profile_bytes(&profile)?;
    let digest = Sha256::digest(&bytes);
    let message = Message::from_digest(digest.into());
    let sig = secp.sign_ecdsa(&message, secret_key);
    Ok(SignedPaymentProfile {
        profile,
        signature: hex::encode(sig.serialize_der()),
    })
}

/// Verify that the signature inside a SignedPaymentProfile is valid.
pub fn verify_signed_profile(signed: &SignedPaymentProfile) -> Result<bool> {
    let secp = Secp256k1::new();

    let pubkey_bytes = hex::decode(&signed.profile.identity_pubkey)
        .map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;
    let public_key = PublicKey::from_slice(&pubkey_bytes)
        .map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;

    let sig_bytes = hex::decode(&signed.signature)
        .map_err(|e| SatsPathError::CryptoError(e.to_string()))?;
    let sig = secp256k1::ecdsa::Signature::from_der(&sig_bytes)
        .map_err(|e| SatsPathError::CryptoError(e.to_string()))?;

    let bytes = canonical_profile_bytes(&signed.profile)?;
    let digest = Sha256::digest(&bytes);
    let message = Message::from_digest(digest.into());

    Ok(secp.verify_ecdsa(&message, &sig, &public_key).is_ok())
}

/// Produce a short human-readable fingerprint of a public key (first 8 hex chars of SHA-256).
pub fn fingerprint_pubkey(pubkey_hex: &str) -> Result<String> {
    let bytes = hex::decode(pubkey_hex)
        .map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;
    let digest = Sha256::digest(&bytes);
    Ok(hex::encode(&digest[..4]))
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
                lnurl: None,
                lightning_address: Some("test@example.com".into()),
                bolt12: None,
            }],
            updated_at: 1_700_000_000,
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
}
