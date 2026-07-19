//! Identity FFI — key management, invites, and key rotation.

use crate::types::*;
use sha2::{Sha256, Digest};

/// Create a new identity keypair. Returns public key, secret key (hex), and fingerprint.
#[uniffi::export]
pub fn create_identity() -> FfiIdentity {
    let kp = satspath_core::crypto::generate_identity_keypair();
    let pubkey_hex = hex::encode(kp.public_key.serialize());
    let fingerprint = compute_fingerprint(&pubkey_hex);
    let secret_key_hex = hex::encode(kp.secret_key.secret_bytes());
    FfiIdentity {
        pubkey_hex,
        secret_key_hex,
        fingerprint,
    }
}

/// Create an invite for an unregistered alias.
#[uniffi::export]
pub fn create_invite(
    alias: String,
    amount_sats: u64,
    sender_secret_key_hex: Option<String>,
    ttl_seconds: i64,
) -> FfiInvite {
    let sender_sk = sender_secret_key_hex
        .and_then(|s| hex::decode(s).ok())
        .and_then(|b| secp256k1::SecretKey::from_slice(&b).ok());
    satspath_core::create_invite(&alias, amount_sats, sender_sk.as_ref(), ttl_seconds).into()
}

/// Create an invite record (for wallet UI display).
#[uniffi::export]
pub fn create_invite_record(
    identifier: String,
    amount_sats: u64,
    memo: Option<String>,
    sender_fingerprint: String,
    ttl_seconds: i64,
) -> FfiInviteRecord {
    satspath_core::create_invite_record(
        &identifier, amount_sats, memo, sender_fingerprint, ttl_seconds,
    ).into()
}

/// Compute SHA-256 identifier hash.
#[uniffi::export]
pub fn identifier_hash(alias: String) -> String {
    satspath_core::privacy::identifier_hash(&alias)
}

/// Mask identifier for display (e.g. "a***@gmail.com").
#[uniffi::export]
pub fn mask_identifier(alias: String) -> String {
    satspath_core::privacy::mask_identifier(&alias)
}

/// Verify a key rotation between old and new profiles.
#[uniffi::export]
pub fn verify_key_rotation(
    old_profile: FfiSignedPaymentProfile,
    new_profile: FfiSignedPaymentProfile,
) -> bool {
    satspath_core::verify_key_rotation(&old_profile.into(), &new_profile.into()).unwrap_or(false)
}

/// Check if a profile's key rotation is valid.
#[uniffi::export]
pub fn is_rotation_valid(profile: FfiSignedPaymentProfile) -> bool {
    satspath_core::is_rotation_valid(&profile.into()).unwrap_or(false)
}

/// Get the effective identity pubkey, considering key rotation.
#[uniffi::export]
pub fn get_effective_identity_pubkey(profile: FfiSignedPaymentProfile) -> String {
    satspath_core::get_effective_identity_pubkey(&profile.into()).unwrap_or_default()
}

/// Compute 16-char fingerprint from compressed pubkey hex.
#[uniffi::export]
pub fn fingerprint_pubkey(pubkey_hex: String) -> String {
    compute_fingerprint(&pubkey_hex)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn compute_fingerprint(pubkey_hex: &str) -> String {
    let Ok(bytes) = hex::decode(pubkey_hex) else {
        return String::new();
    };
    let Ok(pubkey) = secp256k1::PublicKey::from_slice(&bytes) else {
        return String::new();
    };
    let hash = Sha256::digest(pubkey.serialize());
    hex::encode(&hash[..8])
}