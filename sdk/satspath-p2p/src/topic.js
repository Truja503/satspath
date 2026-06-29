// Discovery topic derivation for SatsPath over Holepunch.
//
// A SatsPath alias maps deterministically to a 32-byte DHT topic. The receiver
// announces on this topic; the sender looks it up. The topic reveals only a hash
// of the (already public) alias.

import { sha256 } from "@noble/hashes/sha256";

import { canonicalAlias } from "./profile.js";

const TOPIC_PREFIX = "satspath:p2p:v1:";

/**
 * The 32-byte Hyperswarm/HyperDHT topic for a SatsPath alias.
 * @param {string} alias e.g. "rodrigo@satspath.dev"
 * @returns {Uint8Array} 32 bytes
 */
export function topicForAlias(alias) {
  return sha256(new TextEncoder().encode(TOPIC_PREFIX + canonicalAlias(alias)));
}
