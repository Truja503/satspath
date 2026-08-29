# SatsPath: Current Architecture & Roadmap v0.2

This document summarizes the current state of SatsPath (following 100% compliance with the **SatsPath Protocol v0.1** specification) and outlines features defined for future releases ("Future behavior" / "Production" / "Do not build yet").

---

## 1. Implemented Features (v0.1 Complete + P2P)

The current implementation provides:

- **100% v0.1 Specification Conformance**: Resolvers, Router, signed profiles, key rotation, cryptographic signature verification, and mempool fee evaluation.
- **P2P Decentralized Publishing**: Profiles can be published and resolved over peer-to-peer networks using Holepunch / Pear (`satspath wallet publish`).
- **Multi-Backend Resolution**: Local registry, P2P, HTTPS S2S v2 (`.well-known/satspath-authority`), DNSSEC (BIP-353), and Nostr (NIP-05).
- **Native & Dockerized Daemon & CLI**: Production-ready Rust binaries and container orchestration (`satspath-cli`, `satspathd`, `satspath-witness`).
- **Cryptographic Ownership Proofs**: Method ownership verification preventing impersonation and squatting attacks.

---

## 2. Production Roadmap (v0.2 Milestone)

The following capabilities are scheduled for the v0.2 production milestone:

### A. Real Payment Execution

Currently, the protocol generates validated **Payment Intents** (URIs, QR payloads, BOLT11 invoices) for host wallet handoff. Direct on-chain and node orchestration includes:

- **Integrated Lightning Node Execution (§18.1, §34)**: Automated invoice settlement via embedded node bindings (LDK/Core Lightning).
- **On-Chain Broadcasting (§34)**: Automated PSBT construction, coin selection, hardware signer orchestration, and mempool broadcast.
- **Ark ASP Settlement (§18.3, §34)**: Native VTXO transfers with production Ark Service Providers.
- **Split Payments Batching (§24)**: Multi-recipient recursive resolution and atomic batch settlement.

### B. Privacy & Cryptography

- **Silent Payments (BIP-352) (§13, §18.2)**: On-chain privacy preventing address reuse via reusable Silent Payment public keys.
- **Exact Virtual Size Calculation (§18.2)**: Dynamic PSBT construction for exact byte estimation instead of heuristic defaults.
- **Encrypted Profile Metadata (§26, §27)**: End-to-end encryption of sensitive profile fields using ECDH shared secrets.

### C. Advanced Security & Transparency

- **Automated Witness Quorum Cosigning**: Distributed $K$-of-$N$ witness consensus for real-time split-view defense.
- **Multisig Identity Recovery**: Social and threshold recovery policies for identity key rotation.
- **Cryptographic Domain Attestation (§27)**: Automatic TLS/DNSSEC verification tokens preventing malicious invite links.

### D. Client UX & Mobile Ecosystem

- **Native Mobile SDKs & Wallets (§34)**: React Native and Flutter bindings for iOS/Android sovereign wallets.
- **Full BOLT12 Native Handling (§18.1)**: Direct offer negotiation and payment routing.

---

## Conclusion

SatsPath delivers a zero-trust, proof-carrying resolution layer adhering to the core principle: *"Trust cryptographic signatures, not servers"*. The v0.2 milestone hardens execution bridges and private settlement rails.
