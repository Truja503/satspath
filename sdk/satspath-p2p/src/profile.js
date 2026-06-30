// SatsPath signed-profile verification, native in JS.
//
// Current Rust signs `SHA-256(canonical_json(profile))` with secp256k1 ECDSA
// (DER signature, compressed pubkey), where canonical JSON sorts object keys.
// Older fixtures used Rust's serde insertion order, so verification accepts both
// current canonical JSON and the legacy compact JSON encoding.

import { secp256k1 } from "@noble/curves/secp256k1";
import { sha256 } from "@noble/hashes/sha256";

/** Canonical alias form used for topic derivation and matching. */
export function canonicalAlias(alias) {
  return String(alias).trim().toLowerCase();
}

/** Current canonical JSON bytes: UTF-8 of JSON with object keys sorted. */
export function canonicalProfileBytes(profile) {
  return new TextEncoder().encode(canonicalJson(profile));
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
    const sig = secp256k1.Signature.fromDER(signature);
    const currentHash = sha256(canonicalProfileBytes(signed.profile));
    if (secp256k1.verify(sig, currentHash, pubkey)) return true;

    const legacyHash = sha256(new TextEncoder().encode(JSON.stringify(signed.profile)));
    return secp256k1.verify(sig, legacyHash, pubkey);
  } catch {
    return false;
  }
}

function canonicalJson(value) {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;

  return `{${Object.keys(value)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
    .join(",")}}`;
}
