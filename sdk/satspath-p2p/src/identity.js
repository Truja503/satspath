// Node identity ("address") for the P2P layer.
//
// Each SatsPath P2P node has an auto-generated, random Ed25519 keypair used by
// Hyperswarm's Noise handshake. Its public key (hex) is the node's stable
// **address** on the DHT. This is NOT a Bitcoin key and controls no funds — it
// only identifies the encrypted P2P connection.

import DHT from "hyperdht";

/**
 * Generate a fresh random node keypair.
 * @returns {{ publicKey: Uint8Array, secretKey: Uint8Array }}
 */
export function generateNodeKeyPair() {
  return DHT.keyPair();
}

/** Hex-encode a node public key into its display address. */
export function addressFromPublicKey(publicKey) {
  return Buffer.from(publicKey).toString("hex");
}
