# SatsPath Security Audit Brief

This document serves as a high-level brief for external security researchers auditing the SatsPath core protocol. 
It outlines the key cryptographic assumptions and trust models employed in the system.

## 1. HybridIdentityKeypair (Schnorr + ML-DSA)

SatsPath uses a hybrid signature scheme to bind user profiles to their identity keys, designed to provide both classical and post-quantum resistance.

### Cryptographic Assumptions
- **Classical Resistance**: Provided by secp256k1 Schnorr signatures (BIP-340).
- **Post-Quantum Resistance**: Provided by ML-DSA-65 (formerly Dilithium3), as standardized by NIST FIPS 204.
- **Hybrid Construction**: The profile JSON is canonicalized and hashed (SHA-256). The digest is then signed independently by both the secp256k1 secret key and the ML-DSA secret key. Both signatures must be valid for the profile to be accepted if `pqc_required` is true.

### Audit Scope
Auditors should verify:
1. The canonicalization logic in `canonical_json::to_string` produces identical outputs across platforms, preventing signature malleability.
2. The `hybrid_signature` verification in `satspath-pqc` does not fall back to insecure defaults if the PQC bundle is malformed.

## 2. Ark Zero-Trust Verification

When a user receives funds via the Ark protocol, they advertise a public key and a virtual UTXO (vtxo_pointer).

### Trust Model
- The router (sender) NEVER trusts the Ark Service Provider (ASP) directly.
- Instead, the router fetches the vtxo ownership proof from the ASP.
- The router independently verifies the signature on the Ark proof against the user's advertised public key.
- The router MUST verify that the virtual UTXO has not been spent by checking the latest Ark state tree.

### Audit Scope
Auditors should verify:
1. The `verify_ark_proof()` function correctly asserts ownership without trusting the ASP's HTTP response blindly.
2. The `fetch_fee_estimate` fallback logic never allows an attacker to manipulate the fee oracle to force payments down an insecure rail.

## 3. Nostr Profile Propagation (NIP-01 / NIP-05)

- Profiles are published as NIP-01 Kind 30078 events.
- **Downgrade Attack Protection**: The router queries multiple NIP-05 relays concurrently and selects the profile with the highest `sequence` number.
- **Revocation**: A profile with the `revoked: true` flag and a valid signature permanently tombstones the identity.

### Audit Scope
1. Ensure the concurrent resolution logic in `NostrResolver::resolve_alias` correctly drops lower-sequence profiles even if they are served by the primary relay.
2. Verify that a revoked profile cannot be "un-revoked" by submitting a newer sequence number without the flag.
