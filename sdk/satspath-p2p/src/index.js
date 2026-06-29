// @satspath/p2p — resolve & serve SatsPath signed payment profiles peer-to-peer
// over the Holepunch stack (Hyperswarm / HyperDHT), with NAT hole-punching.
//
// This is a transport + verification layer. It moves only public, signed payment
// profiles. It never moves funds, signs Bitcoin transactions, or broadcasts.

export { SatsPathP2PNode } from "./node.js";
export { generateNodeKeyPair, addressFromPublicKey } from "./identity.js";
export { topicForAlias } from "./topic.js";
export {
  verifySignedProfile,
  canonicalProfileBytes,
  canonicalAlias,
} from "./profile.js";
