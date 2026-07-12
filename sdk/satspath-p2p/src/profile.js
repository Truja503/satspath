// SatsPath signed-profile verification — backed by Rust→WASM.
//
// Replaces @noble/curves/secp256k1 + @noble/hashes/sha256 + manual canonical JSON.
// The WASM module (compiled from satspath-core primitives) runs the exact same
// algorithm as the Rust daemon, eliminating any drift between implementations.
//
// Hyperswarm transport stays in Node.js — only crypto moves to WASM.

import { createRequire } from "module";
const require = createRequire(import.meta.url);

// Lazy-load the WASM module so the file can be imported even before wasm-pack
// has run (useful for unit tests that mock the functions).
let _wasm = null;

async function getWasm() {
  if (_wasm) return _wasm;
  try {
    // wasm-pack --target nodejs emits a CJS bundle with synchronous init
    _wasm = require("../pkg/satspath_wasm.js");
  } catch (e) {
    throw new Error(
      `[satspath-p2p] Could not load WASM module: ${e.message}\n` +
      `  Run: wasm-pack build crates/satspath-wasm --target nodejs --out-dir sdk/satspath-p2p/pkg`
    );
  }
  return _wasm;
}

/** Canonical alias form used for topic derivation and matching. */
export function canonicalAlias(alias) {
  return String(alias).trim().toLowerCase();
}

/**
 * Verify a SatsPath `SignedPaymentProfile` `{ profile, signature }`.
 * Returns `true` only if the secp256k1 ECDSA signature is valid for the
 * profile's `identity_pubkey`. Never throws.
 *
 * Delegates to the Rust WASM implementation — byte-for-byte identical to
 * what `satspathd` does on the server side.
 */
export function verifySignedProfile(signed) {
  try {
    const wasm = getWasmSync();
    return wasm.verify_signed_profile(JSON.stringify(signed));
  } catch {
    return false;
  }
}

/**
 * Return the canonical UTF-8 bytes of a profile (key-sorted JSON).
 * Returns `Uint8Array` — same output as `satspath-core::crypto::canonical_profile_bytes`.
 */
export function canonicalProfileBytes(profile) {
  try {
    const wasm = getWasmSync();
    return wasm.canonical_profile_json(JSON.stringify(profile));
  } catch {
    return new Uint8Array(0);
  }
}

// Synchronous WASM load for Node.js (wasm-pack --target nodejs uses sync init).
function getWasmSync() {
  if (_wasm) return _wasm;
  _wasm = require("../pkg/satspath_wasm.js");
  return _wasm;
}
