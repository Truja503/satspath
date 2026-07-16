//! WASM-exported crypto functions for SatsPath.
//!
//! Replaces `@noble/curves` + `@noble/hashes` in `sdk/satspath-p2p`.
//! Uses the exact same algorithm as `satspath-core::crypto`:
//!   sig = Schnorr(SHA-256("SatsPathProfileV1" || canonical_json(profile)))

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

/// Domain separator — must match satspath-core::crypto::PROFILE_DOMAIN_SEPARATOR
const PROFILE_DOMAIN_SEPARATOR: &[u8] = b"SatsPathProfileV1";

#[derive(Debug, Deserialize, Serialize)]
struct SignedPaymentProfile {
    profile: Value,
    signature: String,
}

/// Verify a SatsPath `SignedPaymentProfile` passed as a JSON string.
///
/// Returns `true` only if the secp256k1 Schnorr signature is valid for the
/// profile's `identity_pubkey`. Returns `false` on any error — never throws.
///
/// Algorithm (matches Protocol v0.1 §12 / satspath-core):
///   digest = SHA-256("SatsPathProfileV1" || canonical_json(profile))
///   verify Schnorr(sig, digest, identity_pubkey)
///
/// Also attempts legacy fallback (insertion-order JSON, no domain separator)
/// for profiles signed by very early satspath-core versions using ECDSA.
#[wasm_bindgen]
pub fn verify_signed_profile(signed_json: &str) -> bool {
    let signed: SignedPaymentProfile = match serde_json::from_str(signed_json) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let pubkey_str = match signed.profile.get("identity_pubkey").and_then(Value::as_str) {
        Some(s) => s,
        None => return false,
    };

    let pubkey_bytes = match hex::decode(pubkey_str) {
        Ok(b) => b,
        Err(_) => return false,
    };

    // Parse pubkey as XOnlyPublicKey (32 bytes) or full PublicKey (33 bytes)
    let x_only_public_key = if pubkey_bytes.len() == 32 {
        match secp256k1::XOnlyPublicKey::from_slice(&pubkey_bytes) {
            Ok(k) => k,
            Err(_) => return false,
        }
    } else {
        match secp256k1::PublicKey::from_slice(&pubkey_bytes) {
            Ok(k) => k.x_only_public_key().0,
            Err(_) => return false,
        }
    };

    let sig_bytes = match hex::decode(&signed.signature) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let sig = match secp256k1::schnorr::Signature::from_slice(&sig_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let secp = secp256k1::Secp256k1::verification_only();

    // Try current canonical JSON (key-sorted) with domain separator
    if let Ok(canonical_bytes) = canonical_json_bytes(&signed.profile) {
        let mut hasher = Sha256::new();
        hasher.update(PROFILE_DOMAIN_SEPARATOR);
        hasher.update(&canonical_bytes);
        let digest = hasher.finalize();
        if let Ok(msg) = secp256k1::Message::from_digest_slice(&digest) {
            if secp.verify_schnorr(&sig, &msg, &x_only_public_key).is_ok() {
                return true;
            }
        }
    }

    // Legacy fallback: insertion-order JSON (no domain separator), ECDSA
    if let Ok(ecdsa_sig) = secp256k1::ecdsa::Signature::from_der(&sig_bytes) {
        if let Ok(pubkey) = secp256k1::PublicKey::from_slice(&pubkey_bytes) {
            let legacy_json = match serde_json::to_string(&signed.profile) {
                Ok(s) => s,
                Err(_) => return false,
            };
            let legacy_digest = Sha256::digest(legacy_json.as_bytes());
            if let Ok(msg) = secp256k1::Message::from_digest_slice(&legacy_digest) {
                return secp.verify_ecdsa(&msg, &ecdsa_sig, &pubkey).is_ok();
            }
        }
    }

    false
}

/// Return the canonical UTF-8 JSON bytes of a profile JSON string.
///
/// Same as `satspath-core::crypto::canonical_profile_bytes` but callable from JS.
/// Returns an empty `Uint8Array` on parse/serialization error.
#[wasm_bindgen]
pub fn canonical_profile_json(profile_json: &str) -> Vec<u8> {
    let value: Value = match serde_json::from_str(profile_json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    canonical_json_bytes(&value).unwrap_or_default()
}

/// Compute the 8-char fingerprint of a compressed secp256k1 pubkey.
///
/// Returns the first 8 hex characters (matching Rust `fingerprint_pubkey`).
/// Returns empty string on invalid input.
#[wasm_bindgen]
pub fn fingerprint_pubkey(pubkey_hex: &str) -> String {
    let bytes = match hex::decode(pubkey_hex) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    if bytes.len() != 33 {
        return String::new();
    }
    // Skip the 02/03 prefix, take first 4 bytes = 8 hex chars
    hex::encode(&bytes[1..5])
}

fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, String> {
    canonical_json::to_string(value)
        .map(|s| s.into_bytes())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_matches_rust() {
        let pk = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        assert_eq!(fingerprint_pubkey(pk), "79be667e");
    }

    #[test]
    fn canonical_json_deterministic() {
        let a = r#"{"b":1,"a":2}"#;
        let b = r#"{"a":2,"b":1}"#;
        assert_eq!(canonical_profile_json(a), canonical_profile_json(b));
    }
}