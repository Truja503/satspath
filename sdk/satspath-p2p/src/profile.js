// SatsPath signed-profile verification, native in JS.
//
// SatsPath signs `SHA-256(serde_json::to_string(profile))` with secp256k1 ECDSA
// (DER signature, compressed pubkey). JavaScript preserves object key insertion
// order, so `JSON.stringify(profile)` of a profile parsed from `satspath export`
// reproduces Rust's exact canonical bytes — verified against a real Rust
// signature in the test suite. No re-implementation of serde is required.

import { secp256k1 } from "@noble/curves/secp256k1";
import { sha256 } from "@noble/hashes/sha256";

/** Canonical alias form used for topic derivation and matching. */
export function canonicalAlias(alias) {
  return String(alias).trim().toLowerCase();
}

/** The exact bytes SatsPath signed: UTF-8 of compact JSON, in field order. */
export function canonicalProfileBytes(profile) {
  return new TextEncoder().encode(JSON.stringify(profile));
}

/**
 * Verify a SatsPath `SignedPaymentProfile` `{ profile, signature }`.
 * Returns `true` only if the secp256k1 signature is valid for the profile's
 * `identity_pubkey`. Never throws.
 */
export function verifySignedProfile(signed) {
  try {
    const pubkey = signed?.profile?.identity_pubkey;
    const signature = signed?.signature;
    if (typeof pubkey !== "string" || typeof signature !== "string") return false;
    const msgHash = sha256(canonicalProfileBytes(signed.profile));
    const sig = secp256k1.Signature.fromDER(signature);
    return secp256k1.verify(sig, msgHash, pubkey);
  } catch {
    return false;
  }
}
