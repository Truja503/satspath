//! `satspath-wasm` — Minimal WASM bindings for SatsPath.
//!
//! Provides:
//!   - `verify_signed_profile` — secp256k1 Schnorr verification
//!   - `canonical_profile_json` — deterministic canonical JSON bytes
//!   - `fingerprint_pubkey` — 8-char pubkey fingerprint
//!   - `topic_for_alias` — SHA-256 P2P topic derivation
//!   - `resolve_alias` — resolver chain: local → BIP353 → HTTPS well-known → Nostr NIP-05
//!   - `quote` — resolve + verify + route + build QR payload
//!   - `build_qr_payload` — payment URI builder (BOLT11, BIP21, ark:)
//!
//! All dependencies are WASM-compatible (no tokio, reqwest, mio).

use wasm_bindgen::prelude::*;

mod crypto;
mod helpers;
mod resolver;
mod router;
mod topic;
mod types;

pub use crypto::{canonical_profile_json, verify_signed_profile, fingerprint_pubkey};
pub use helpers::{identifier_hash, mask_identifier};
pub use topic::topic_for_alias;
pub use resolver::{ChainResolver, LocalRegistry, Bip353Resolver, HttpsWellKnownResolver, NostrNip05Resolver};
pub use router::{quote, build_qr_payload, select_route, select_route_live, fetch_fee_estimate};
pub use types::{
    SignedPaymentProfile, PaymentProfile, PaymentMethod, BitcoinNetwork, FeeEstimate, FeeRateSnapshot,
    RouteRequest, Invite, SwapDirective, PaymentUrgency, QuoteRecipient, FALLBACK_FEES,
    LIGHTNING_THRESHOLD_SATS, ONCHAIN_FEE_THRESHOLD_SAT_VB, ONCHAIN_FEE_BUFFER,
    KeyRotation, MethodVerification, ArkOwnershipProof,
    QuoteResponse, RouteQuote, ExecutionMode,
};

/// Initialize the WASM module (better panic messages in JS console).
/// Call once at startup in Node.js: `init()`.
#[wasm_bindgen(start)]
pub fn main() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}