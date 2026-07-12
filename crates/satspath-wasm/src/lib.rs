//! `satspath-wasm` — WASM bindings for SatsPath crypto primitives.
//!
//! Exposes to JavaScript (Node.js target):
//!   - `verify_signed_profile`  — secp256k1 ECDSA verification
//!   - `canonical_profile_json` — deterministic canonical JSON bytes
//!   - `topic_for_alias`        — SHA-256 P2P topic derivation
//!
//! These replace `@noble/curves` + `@noble/hashes` in `sdk/satspath-p2p`.
//! Hyperswarm/HyperDHT transport stays in Node.js — only crypto moves here.

use wasm_bindgen::prelude::*;

mod crypto;
mod topic;

pub use crypto::{canonical_profile_json, verify_signed_profile};
pub use topic::topic_for_alias;

/// Initialize the WASM module (better panic messages in JS console).
/// Call once at startup in Node.js: `init()`.
#[wasm_bindgen(start)]
pub fn main() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}
