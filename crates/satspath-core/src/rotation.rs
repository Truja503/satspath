use serde::{Deserialize, Serialize};

use crate::crypto::{verify_message_signature, sign_message};
use crate::Result;

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
        let message = format!("RotateSatsPathKey:{}->{}", previous_pubkey_hex, new_pubkey_hex);
        let signature = sign_message(&message, old_secret_key);
        Ok(Self {
            previous_pubkey: previous_pubkey_hex,
            new_pubkey: new_pubkey_hex,
            authorization_signature: signature,
            rotated_at: chrono::Utc::now().timestamp(),
        })
    }

    /// Verify the rotation authorization signature.
    pub fn verify(&self) -> Result<bool> {
        let message = format!("RotateSatsPathKey:{}->{}", self.previous_pubkey, self.new_pubkey);
        verify_message_signature(&message, &self.authorization_signature, &self.previous_pubkey)
    }
}
