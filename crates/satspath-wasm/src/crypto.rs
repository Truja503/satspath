//! WASM-exported crypto functions for `satspath-p2p`.
//!
//! Replaces `@noble/curves` and `@noble/hashes` usage in:
//!   - `sdk/satspath-p2p/src/profile.js` (verifySignedProfile, canonicalProfileBytes)
//!
//! Uses the exact same algorithm as `satspath-core::crypto`:
//!   sig = Schnorr(SHA-256("SatsPathProfileV1" || canonical_json(profile)))
//!
//! Note: we cannot depend on `satspath-core` directly from WASM because it pulls
//! in `tokio`, `reqwest`, and `tokio-tungstenite` which don't compile to
//! `wasm32-unknown-unknown`. We use the same underlying crates (secp256k1, sha2,
//! canonical_json) at the same versions.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

/// Domain separator — must match satspath-core::crypto::PROFILE_DOMAIN_SEPARATOR
const PROFILE_DOMAIN_SEPARATOR: &[u8] = b"SatsPathProfileV1";

// ── Minimal profile types needed for deserialization ─────────────────────────

#[derive(Debug, Deserialize, Serialize)]
struct SignedPaymentProfile {
    profile: Value,
    signature: String,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Produce deterministic canonical JSON (keys sorted at every level).
/// Matches `canonical_json::to_string()` used in satspath-core.
fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, String> {
    canonical_json::to_string(value)
        .map(|s| s.into_bytes())
        .map_err(|e| e.to_string())
}

// ── WASM exports ─────────────────────────────────────────────────────────────

/// Verify a SatsPath `SignedPaymentProfile` passed as a JSON string.
///
/// Returns `true` only if the secp256k1 Schnorr signature is valid for the
/// profile's `identity_pubkey`. Returns `false` on any error — never throws.
///
/// Algorithm (matches Protocol v0.1 §12 / satspath-core):
///   digest = SHA-256("SatsPathProfileV1" || canonical_json(profile))
///   verify Schnorr(sig, digest, identity_pubkey)
///
/// Also attempts the legacy fallback (JS insertion-order JSON) if canonical
/// verification fails, so old profiles signed before key-sorting was enforced
/// still pass.
///
/// # Example (Node.js)
/// ```js
/// import { verify_signed_profile } from './pkg/satspath_wasm.js';
/// const ok = verify_signed_profile(JSON.stringify(signedProfile)); // true/false
/// ```
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

    // ── Try current canonical JSON (key-sorted) ───────────────────────────
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

    // ── Legacy fallback: insertion-order JSON (no domain separator) ───────
    // This covers profiles signed by very early satspath-core versions using ECDSA.
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
///
/// # Example (Node.js)
/// ```js
/// import { canonical_profile_json } from './pkg/satspath_wasm.js';
/// const bytes = canonical_profile_json(JSON.stringify(profile));
/// ```
#[wasm_bindgen]
pub fn canonical_profile_json(profile_json: &str) -> Vec<u8> {
    let value: Value = match serde_json::from_str(profile_json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    canonical_json_bytes(&value).unwrap_or_default()
}
