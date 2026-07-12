// Discovery topic derivation for SatsPath over Holepunch — backed by Rust→WASM.
//
// Replaces @noble/hashes/sha256 usage. The WASM implementation uses sha2 (same
// crate as satspath-core) so topics are byte-for-byte identical with any peer,
// regardless of whether they derive the topic from Rust or this JS module.

import { createRequire } from "module";
const require = createRequire(import.meta.url);

import { canonicalAlias } from "./profile.js";

let _wasm = null;

function getWasm() {
  if (_wasm) return _wasm;
  try {
    _wasm = require("../pkg/satspath_wasm.js");
  } catch (e) {
    throw new Error(
      `[satspath-p2p] Could not load WASM module: ${e.message}\n` +
      `  Run: wasm-pack build crates/satspath-wasm --target nodejs --out-dir sdk/satspath-p2p/pkg`
    );
  }
  return _wasm;
}

const TOPIC_PREFIX = "satspath:p2p:v1:";

/**
 * The 32-byte Hyperswarm/HyperDHT topic for a SatsPath alias.
 *
 * Delegates to the Rust WASM implementation:
 *   SHA-256("satspath:p2p:v1:" + alias.trim().toLowerCase())
 *
 * @param {string} alias e.g. "rodrigo@satspath.dev"
 * @returns {Buffer} 32 bytes — same result as the Rust implementation
 */
export function topicForAlias(alias) {
  const wasm = getWasm();
  return Buffer.from(wasm.topic_for_alias(alias));
}
