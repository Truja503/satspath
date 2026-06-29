use secp256k1::{ecdsa::Signature, Message, PublicKey, Secp256k1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::errors::{Result, SatsPathError};
use crate::validation::{assert_no_private_material, validate_compressed_pubkey};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArkRouteKind {
    ArkToArk,
    ArkToLightning,
    LightningToArk,
    ArkToOnchain,
    OnchainToArk,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArkReceivePointer {
    pub server: String,
    pub receiver_pubkey: String,
    pub vtxo_pointer: Option<String>,
    pub proof: Option<ArkOwnershipProof>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArkOwnershipProof {
    pub message: String,
    pub signature: String,
    pub pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArkIntentStatus {
    PreviewOnly,
    TestnetReady,
    TestnetExecutionBlocked,
    ExecutedTestnet,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArkPaymentIntent {
    pub route_kind: ArkRouteKind,
    pub amount_sats: u64,
    pub receiver_pointer: ArkReceivePointer,
    pub status: ArkIntentStatus,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientValidationReport {
    pub profile_signature_valid: bool,
    pub profile_fresh: bool,
    pub ark_pointer_valid: bool,
    pub ark_ownership_verified: bool,
    pub method_verified: bool,
    pub safe_for_preview: bool,
    pub safe_for_testnet_execution: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

pub fn ark_ownership_challenge(
    alias: &str,
    ark_server: &str,
    receiver_pubkey: &str,
    nonce: &str,
) -> String {
    format!("satspath-proof:{alias}:ark:{ark_server}:{receiver_pubkey}:{nonce}")
}

pub fn validate_ark_server_url(server: &str) -> Result<()> {
    assert_no_private_material(server)?;
    let parsed =
        Url::parse(server).map_err(|e| SatsPathError::InvalidPaymentPointer(e.to_string()))?;
    if parsed.scheme() != "https" {
        return Err(SatsPathError::InvalidPaymentPointer(
            "Ark server URL must use https".into(),
        ));
    }
    if parsed.host_str().is_none() {
        return Err(SatsPathError::InvalidPaymentPointer(
            "Ark server URL must include a host".into(),
        ));
    }
    Ok(())
}

pub fn validate_ark_receive_pointer(pointer: &ArkReceivePointer, now: i64) -> Result<()> {
    validate_ark_server_url(&pointer.server)?;
    validate_compressed_pubkey(&pointer.receiver_pubkey)?;
    if let Some(vtxo) = &pointer.vtxo_pointer {
        assert_no_private_material(vtxo)?;
    }
    if let Some(expires_at) = pointer.expires_at {
        if expires_at <= now {
            return Err(SatsPathError::InvalidPaymentPointer(
                "Ark receive pointer expired".into(),
            ));
        }
    }
    Ok(())
}

pub fn verify_ark_ownership_proof(
    alias: &str,
    pointer: &ArkReceivePointer,
    now: i64,
) -> Result<bool> {
    validate_ark_receive_pointer(pointer, now)?;
    let Some(proof) = &pointer.proof else {
        return Ok(false);
    };
    validate_compressed_pubkey(&proof.pubkey)?;
    if proof.pubkey != pointer.receiver_pubkey {
        return Err(SatsPathError::InvalidSignature);
    }
    if let Some(expires_at) = pointer.expires_at {
        if expires_at <= now {
            return Err(SatsPathError::InvalidPaymentPointer(
                "Ark ownership proof expired".into(),
            ));
        }
    }

    let expected_prefix =
        ark_ownership_challenge(alias, &pointer.server, &pointer.receiver_pubkey, "");
    if !proof.message.starts_with(&expected_prefix) || proof.message == expected_prefix {
        return Err(SatsPathError::InvalidSignature);
    }

    let pubkey_bytes =
        hex::decode(&proof.pubkey).map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;
    let pubkey = PublicKey::from_slice(&pubkey_bytes)
        .map_err(|e| SatsPathError::InvalidPublicKey(e.to_string()))?;
    let sig_bytes =
        hex::decode(&proof.signature).map_err(|e| SatsPathError::CryptoError(e.to_string()))?;
    let sig =
        Signature::from_der(&sig_bytes).map_err(|e| SatsPathError::CryptoError(e.to_string()))?;
    let digest = Sha256::digest(proof.message.as_bytes());
    let message = Message::from_digest(digest.into());
    Ok(Secp256k1::verification_only()
        .verify_ecdsa(&message, &sig, &pubkey)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::{Secp256k1, SecretKey};

    fn signed_pointer(
        alias: &str,
        server: &str,
        nonce: &str,
        expires_at: Option<i64>,
    ) -> ArkReceivePointer {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let pubkey = secret.public_key(&secp);
        let pubkey_hex = hex::encode(pubkey.serialize());
        let message = ark_ownership_challenge(alias, server, &pubkey_hex, nonce);
        let digest = Sha256::digest(message.as_bytes());
        let sig = secp.sign_ecdsa(&Message::from_digest(digest.into()), &secret);
        ArkReceivePointer {
            server: server.into(),
            receiver_pubkey: pubkey_hex.clone(),
            vtxo_pointer: None,
            proof: Some(ArkOwnershipProof {
                message,
                signature: hex::encode(sig.serialize_der()),
                pubkey: pubkey_hex,
            }),
            expires_at,
        }
    }

    #[test]
    fn valid_proof_accepted() {
        let pointer = signed_pointer(
            "alice@example.com",
            "https://ark.example.com",
            "n1",
            Some(2_000),
        );
        assert!(verify_ark_ownership_proof("alice@example.com", &pointer, 1_000).unwrap());
    }

    #[test]
    fn pubkey_incorrect_rejected() {
        let mut pointer = signed_pointer(
            "alice@example.com",
            "https://ark.example.com",
            "n1",
            Some(2_000),
        );
        pointer.receiver_pubkey =
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".into();
        assert!(verify_ark_ownership_proof("alice@example.com", &pointer, 1_000).is_err());
    }

    #[test]
    fn server_incorrect_rejected() {
        let mut pointer = signed_pointer(
            "alice@example.com",
            "https://ark.example.com",
            "n1",
            Some(2_000),
        );
        pointer.server = "https://evil.example.com".into();
        assert!(verify_ark_ownership_proof("alice@example.com", &pointer, 1_000).is_err());
    }

    #[test]
    fn alias_incorrect_rejected() {
        let pointer = signed_pointer(
            "alice@example.com",
            "https://ark.example.com",
            "n1",
            Some(2_000),
        );
        assert!(verify_ark_ownership_proof("bob@example.com", &pointer, 1_000).is_err());
    }

    #[test]
    fn malformed_signature_rejected() {
        let mut pointer = signed_pointer(
            "alice@example.com",
            "https://ark.example.com",
            "n1",
            Some(2_000),
        );
        pointer.proof.as_mut().unwrap().signature = "not-hex".into();
        assert!(verify_ark_ownership_proof("alice@example.com", &pointer, 1_000).is_err());
    }

    #[test]
    fn proof_expired_rejected() {
        let pointer = signed_pointer(
            "alice@example.com",
            "https://ark.example.com",
            "n1",
            Some(999),
        );
        assert!(verify_ark_ownership_proof("alice@example.com", &pointer, 1_000).is_err());
    }
}
