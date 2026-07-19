//! Crypto FFI — signing, verification, and profile crypto operations.

use crate::types::*;

/// Sign a message with a secret key (hex).
#[uniffi::export]
pub fn sign_message(message: String, secret_key_hex: String) -> Result<String, FfiError> {
    let sk_bytes = hex::decode(&secret_key_hex)
        .map_err(|e| FfiError::CryptoError { reason: e.to_string() })?;
    let sk = secp256k1::SecretKey::from_slice(&sk_bytes)
        .map_err(|e| FfiError::CryptoError { reason: e.to_string() })?;
    Ok(satspath_core::crypto::sign_message(&message, &sk))
}

/// Verify a message signature.
#[uniffi::export]
pub fn verify_message_signature(
    message: String,
    signature_hex: String,
    pubkey_hex: String,
) -> Result<bool, FfiError> {
    satspath_core::crypto::verify_message_signature(&message, &signature_hex, &pubkey_hex)
        .map_err(|e| FfiError::CryptoError { reason: e.to_string() })
}

/// Verify a signed payment profile's signature.
#[uniffi::export]
pub fn verify_signed_profile(profile: FfiSignedPaymentProfile) -> bool {
    satspath_core::verify_signed_profile(&profile.into()).unwrap_or(false)
}

/// Sign a payment profile with an identity secret key.
#[uniffi::export]
pub fn sign_profile(
    profile: FfiPaymentProfile,
    secret_key_hex: String,
) -> Result<FfiSignedPaymentProfile, FfiError> {
    let sk_bytes = hex::decode(&secret_key_hex)
        .map_err(|e| FfiError::CryptoError { reason: e.to_string() })?;
    let sk = secp256k1::SecretKey::from_slice(&sk_bytes)
        .map_err(|e| FfiError::CryptoError { reason: e.to_string() })?;
    satspath_core::sign_profile(profile.into(), &sk)
        .map(Into::into)
        .map_err(|e| FfiError::CryptoError { reason: e.to_string() })
}