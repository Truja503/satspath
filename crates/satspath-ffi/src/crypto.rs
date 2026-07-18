//! Crypto FFI wrappers

use satspath_core::crypto::{
    fingerprint_pubkey, verify_signed_profile, sign_message, verify_message_signature,
    generate_identity_keypair, sign_profile,
};
use satspath_core::PaymentProfile;
use uniffi::Object;

/// Generate a new identity keypair
#[uniffi::export]
pub fn generate_identity_keypair() -> IdentityKeypair {
    let kp = generate_identity_keypair();
    IdentityKeypair {
        pubkey_hex: hex::encode(kp.public_key.serialize()),
        secret_key_hex: hex::encode(kp.secret_key.secret_bytes()),
    }
}

/// Compute 8-char fingerprint from compressed pubkey hex
#[uniffi::export]
pub fn fingerprint_pubkey(pubkey_hex: String) -> String {
    fingerprint_pubkey(&pubkey_hex).unwrap_or_default()
}

/// Verify a signed payment profile
#[uniffi::export]
pub fn verify_profile(profile: SignedPaymentProfile) -> bool {
    verify_signed_profile(&profile.into()).unwrap_or(false)
}

/// Sign a message with a secret key (hex)
#[uniffi::export]
pub fn sign_message(message: String, secret_key_hex: String) -> Result<String, String> {
    let sk_bytes = hex::decode(secret_key_hex).map_err(|e| e.to_string())?;
    let sk = secp256k1::SecretKey::from_slice(&sk_bytes).map_err(|e| e.to_string())?;
    Ok(sign_message(&message, &sk))
}

/// Verify a message signature
#[uniffi::export]
pub fn verify_message(message: String, signature_hex: String, pubkey_hex: String) -> Result<bool, String> {
    verify_message_signature(&message, &signature_hex, &pubkey_hex).map_err(|e| e.to_string())
}

/// Sign a PaymentProfile with identity secret key
#[uniffi::export]
pub fn sign_profile(profile: PaymentProfile, secret_key_hex: String) -> Result<SignedPaymentProfile, String> {
    let sk_bytes = hex::decode(secret_key_hex).map_err(|e| e.to_string())?;
    let sk = secp256k1::SecretKey::from_slice(&sk_bytes).map_err(|e| e.to_string())?;
    let signed = sign_profile(profile, &sk).map_err(|e| e.to_string())?;
    Ok(signed.into())
}

/// Identity keypair for FFI
#[derive(uniffi::Record)]
pub struct IdentityKeypair {
    pub pubkey_hex: String,
    pub secret_key_hex: String,
}

/// SignedPaymentProfile for FFI
#[derive(uniffi::Record)]
pub struct SignedPaymentProfile {
    pub profile: PaymentProfile,
    pub signature: String,
}

/// PaymentProfile for FFI
#[derive(uniffi::Record)]
pub struct PaymentProfile {
    pub alias: String,
    pub identity_pubkey: String,
    pub methods: Vec<PaymentMethod>,
    pub updated_at: i64,
    pub expires_at: Option<i64>,
    pub sequence: Option<u64>,
    pub preferences: Vec<String>,
    pub nonce: Option<String>,
    pub rotation: Option<KeyRotation>,
    pub method_verifications: Vec<MethodVerification>,
}

/// PaymentMethod for FFI
#[derive(uniffi::Enum)]
pub enum PaymentMethod {
    Onchain(OnchainMethod),
    Lightning(LightningMethod),
    Ark(ArkMethod),
}

/// OnchainMethod for FFI
#[derive(uniffi::Record)]
pub struct OnchainMethod {
    pub label: String,
    pub network: String,
    pub address: Option<String>,
    pub silent_payment_pubkey: Option<String>,
    pub pubkey_hint: Option<String>,
    pub descriptor_hint: Option<String>,
    pub address_list: Vec<String>,
}

/// LightningMethod for FFI
#[derive(uniffi::Record)]
pub struct LightningMethod {
    pub label: String,
    pub lightning_address: Option<String>,
    pub lnurl: Option<String>,
    pub bolt12: Option<String>,
    pub receiver_pubkey: Option<String>,
}

/// ArkMethod for FFI
#[derive(uniffi::Record)]
pub struct ArkMethod {
    pub label: String,
    pub server: String,
    pub pubkey: String,
    pub vtxo_pointer: Option<String>,
    pub opaque_uri: Option<String>,
    pub proof: Option<ArkOwnershipProof>,
    pub expires_at: Option<i64>,
}

/// ArkOwnershipProof for FFI
#[derive(uniffi::Record)]
pub struct ArkOwnershipProof {
    pub message: String,
    pub signature: String,
    pub pubkey: String,
}

/// KeyRotation for FFI
#[derive(uniffi::Record)]
pub struct KeyRotation {
    pub new_identity_pubkey: String,
    pub rotation_time: i64,
    pub previous_signature: String,
}

/// MethodVerification for FFI
#[derive(uniffi::Record)]
pub struct MethodVerification {
    pub method_descriptor: String,
    pub proof_type: String,
    pub proof_data: String,
    pub verified_at: i64,
}