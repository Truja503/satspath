//! Identity FFI implementation

use satspath_core::crypto::generate_identity_keypair;
use satspath_core::privacy::{identifier_hash, mask_identifier};
use satspath_core::profile::{create_invite, create_invite_record, Invite, InviteRecord, InviteStatus};
use satspath_core::rotation::{apply_key_rotation, get_effective_identity_pubkey, is_rotation_valid, rotate_identity_key, verify_key_rotation};
use satspath_core::{crypto::{sign_profile, verify_signed_profile}, SatsPathError};
use secp256k1::SecretKey;
use std::str::FromStr;

use uniffi::deps::anyhow::Result;

// Use generated FFI types
use crate::{
    Identity as FfiIdentity,
    Invite as FfiInvite,
    InviteRecord as FfiInviteRecord,
    InviteStatus as FfiInviteStatus,
    SignedPaymentProfile as FfiSignedPaymentProfile,
    PaymentProfile as FfiPaymentProfile,
    FfiError,
};

/// Create a new identity (keypair + fingerprint)
#[uniffi::export]
pub fn create_identity() -> crate::Identity {
    let kp = satspath_core::generate_identity_keypair();
    let pubkey_hex = hex::encode(kp.public_key.serialize());
    let fingerprint = fingerprint_pubkey(&pubkey_hex).unwrap_or_default();
    
    // In production, save to platform keystore (Android Keystore, iOS Keychain, etc.)
    // For now, return path where it would be saved
    let secret_key_path = format!(".satspath/identity/{}.key", fingerprint);
    
    crate::Identity {
        pubkeyHex: pubkey_hex,
        secretKeyPath: secret_key_path,
        fingerprint,
    }
}

/// Create an invite for an unregistered alias
#[uniffi::export]
pub fn create_invite_ffi(alias: String, amount_sats: u64, sender_secret_key_hex: Option<String>, ttl_seconds: i64) -> crate::Invite {
    let sender_sk = sender_secret_key_hex
        .and_then(|s| hex::decode(s).ok())
        .and_then(|b| SecretKey::from_slice(&b).ok());
    
    let invite = satspath_core::create_invite(&alias, amount_sats, sender_sk.as_ref(), ttl_seconds);
    invite.into()
}

/// Create an invite record (for wallet UI)
#[uniffi::export]
pub fn create_invite_record_ffi(identifier: String, amount_sats: u64, memo: Option<String>, sender_fingerprint: String, ttl_seconds: i64) -> crate::InviteRecord {
    satspath_core::create_invite_record(&identifier, amount_sats, memo, sender_fingerprint, ttl_seconds).into()
}

/// Compute identifier hash
#[uniffi::export]
pub fn identifier_hash_ffi(alias: String) -> String {
    satspath_core::privacy::identifier_hash(&alias)
}

/// Mask identifier for display
#[uniffi::export]
pub fn mask_identifier_ffi(alias: String) -> String {
    satspath_core::privacy::mask_identifier(&alias)
}

/// Verify a signed profile
#[uniffi::export]
pub fn verify_profile_ffi(profile: crate::SignedPaymentProfile) -> bool {
    satspath_core::verify_signed_profile(&profile.into()).unwrap_or(false)
}

/// Sign a profile with a secret key
#[uniffi::export]
pub fn sign_profile_ffi(profile: crate::PaymentProfile, secret_key_hex: String) -> Result<crate::SignedPaymentProfile, crate::FfiError> {
    let sk = SecretKey::from_str(&secret_key_hex).map_err(|e| crate::FfiError::CryptoError(e.to_string()))?;
    satspath_core::sign_profile(profile.into(), &sk).map(Into::into).map_err(Into::into)
}

/// Verify a key rotation
#[uniffi::export]
pub fn verify_rotation_ffi(old_profile: crate::SignedPaymentProfile, new_profile: crate::SignedPaymentProfile) -> bool {
    satspath_core::verify_key_rotation(&old_profile.into(), &new_profile.into()).unwrap_or(false)
}

/// Apply key rotation to a profile
#[uniffi::export]
pub fn apply_key_rotation_ffi(profile: crate::SignedPaymentProfile, new_pubkey_hex: String) -> Result<crate::SignedPaymentProfile, crate::FfiError> {
    let new_pubkey = secp256k1::PublicKey::from_str(&new_pubkey_hex)
        .map_err(|e| crate::FfiError::CryptoError(e.to_string()))?;
    satspath_core::apply_key_rotation(&profile.into(), &new_pubkey).map(Into::into).map_err(Into::into)
}

/// Check if key rotation is valid
#[uniffi::export]
pub fn is_rotation_valid_ffi(profile: crate::SignedPaymentProfile) -> bool {
    satspath_core::is_rotation_valid(&profile.into()).unwrap_or(false)
}

/// Rotate identity key
#[uniffi::export]
pub fn rotate_identity_key_ffi(profile: crate::SignedPaymentProfile, new_secret_key_hex: String) -> Result<crate::SignedPaymentProfile, crate::FfiError> {
    let new_sk = SecretKey::from_str(&new_secret_key_hex)
        .map_err(|e| crate::FfiError::CryptoError(e.to_string()))?;
    satspath_core::rotate_identity_key(&profile.into(), &new_sk).map(Into::into).map_err(Into::into)
}

/// Get effective identity pubkey (considering rotation)
#[uniffi::export]
pub fn get_effective_identity_pubkey_ffi(profile: crate::SignedPaymentProfile) -> String {
    satspath_core::get_effective_identity_pubkey(&profile.into()).unwrap_or_default()
}

/// Rotate identity key
#[uniffi::export]
pub fn rotate_identity_key_ffi(profile: crate::SignedPaymentProfile, new_secret_key_hex: String) -> Result<crate::SignedPaymentProfile, crate::FfiError> {
    let new_sk = SecretKey::from_str(&new_secret_key_hex)
        .map_err(|e| crate::FfiError::CryptoError(e.to_string()))?;
    satspath_core::rotate_identity_key(&profile.into(), &new_sk).map(Into::into).map_err(Into::into)
}

/// Compute fingerprint from pubkey
fn fingerprint_pubkey(pubkey_hex: &str) -> Result<String, satspath_core::SatsPathError> {
    let pubkey_bytes = hex::decode(pubkey_hex).map_err(|e| satspath_core::SatsPathError::CryptoError(e.to_string()))?;
    let pubkey = secp256k1::PublicKey::from_slice(&pubkey_bytes)
        .map_err(|e| satspath_core::SatsPathError::CryptoError(e.to_string()))?;
    let sha = sha2::Sha256::digest(&pubkey.serialize());
    Ok(hex::encode(&sha[..8]))
}

/// Verify a signed profile
#[uniffi::export]
pub fn verify_profile_ffi(profile: crate::SignedPaymentProfile) -> bool {
    satspath_core::verify_signed_profile(&profile.into()).unwrap_or(false)
}

/// Sign a profile with a secret key
#[uniffi::export]
pub fn sign_profile_ffi(profile: crate::PaymentProfile, secret_key_hex: String) -> Result<crate::SignedPaymentProfile, crate::FfiError> {
    let sk = SecretKey::from_str(&secret_key_hex).map_err(|e| crate::FfiError::CryptoError(e.to_string()))?;
    satspath_core::sign_profile(profile.into(), &sk).map(Into::into).map_err(Into::into)
}

/// Verify a key rotation
#[uniffi::export]
pub fn verify_rotation_ffi(old_profile: crate::SignedPaymentProfile, new_profile: crate::SignedPaymentProfile) -> bool {
    satspath_core::verify_key_rotation(&old_profile.into(), &new_profile.into()).unwrap_or(false)
}

/// Apply key rotation to a profile
#[uniffi::export]
pub fn apply_key_rotation_ffi(profile: crate::SignedPaymentProfile, new_pubkey_hex: String) -> Result<crate::SignedPaymentProfile, crate::FfiError> {
    let new_pubkey = secp256k1::PublicKey::from_str(&new_pubkey_hex)
        .map_err(|e| crate::FfiError::CryptoError(e.to_string()))?;
    satspath_core::apply_key_rotation(&profile.into(), &new_pubkey).map(Into::into).map_err(Into::into)
}

/// Check if key rotation is valid
#[uniffi::export]
pub fn is_rotation_valid_ffi(profile: crate::SignedPaymentProfile) -> bool {
    satspath_core::is_rotation_valid(&profile.into()).unwrap_or(false)
}

/// Rotate identity key
#[uniffi::export]
pub fn rotate_identity_key_ffi(profile: crate::SignedPaymentProfile, new_secret_key_hex: String) -> Result<crate::SignedPaymentProfile, crate::FfiError> {
    let new_sk = SecretKey::from_str(&new_secret_key_hex)
        .map_err(|e| crate::FfiError::CryptoError(e.to_string()))?;
    satspath_core::rotate_identity_key(&profile.into(), &new_sk).map(Into::into).map_err(Into::into)
}

/// Get effective identity pubkey (considering rotation)
#[uniffi::export]
pub fn get_effective_identity_pubkey_ffi(profile: crate::SignedPaymentProfile) -> String {
    satspath_core::get_effective_identity_pubkey(&profile.into()).unwrap_or_default()
}

/// Rotate identity key
#[uniffi::export]
pub fn rotate_identity_key_ffi(profile: crate::SignedPaymentProfile, new_secret_key_hex: String) -> Result<crate::SignedPaymentProfile, crate::FfiError> {
    let new_sk = SecretKey::from_str(&new_secret_key_hex)
        .map_err(|e| crate::FfiError::CryptoError(e.to_string()))?;
    satspath_core::rotate_identity_key(&profile.into(), &new_sk).map(Into::into).map_err(Into::into)
}

/// Compute fingerprint from pubkey
#[uniffi::export]
pub fn fingerprint_pubkey_ffi(pubkey_hex: String) -> String {
    fingerprint_pubkey(&pubkey_hex).unwrap_or_default()
}

/// Compute fingerprint from pubkey
fn fingerprint_pubkey(pubkey_hex: &str) -> Result<String, satspath_core::SatsPathError> {
    let pubkey_bytes = hex::decode(pubkey_hex).map_err(|e| satspath_core::SatsPathError::CryptoError(e.to_string()))?;
    let pubkey = secp256k1::PublicKey::from_slice(&pubkey_bytes)
        .map_err(|e| satspath_core::SatsPathError::CryptoError(e.to_string()))?;
    let sha = sha2::Sha256::digest(&pubkey.serialize());
    Ok(hex::encode(&sha[..8]))
}