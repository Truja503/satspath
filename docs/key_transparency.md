# SatsPath Key Transparency Architecture

## Problem

`self-signed profile != authenticated human-readable identity`. A malicious registry can replace Alice's key with an attacker key and return a profile correctly self-signed by that attacker. Profile-signature validity alone cannot detect the substitution.

V1 adds a per-identifier hash chain inside a global append-only Merkle log, signed checkpoints, client pinning, dual-signature rotation, identifier-to-key attestations and an opt-in Bitcoin Core regtest anchor. The authenticated current-state map is not implemented: state is replayed from the log and `map_root` is `null`.

## Event creation and Merkle construction

```mermaid
flowchart LR
  I[Canonical identifier] --> H[SHA-256 identifier hash]
  P[Canonical signed profile] --> PH[Profile hash]
  H --> E[Versioned NameEvent]
  PH --> E
  K[Authorized identity key] -->|Schnorr owner signature| E
  E --> F[fsync JSONL append]
  F --> T[RFC6962-style Merkle tree]
  T --> C[Operator-signed checkpoint]
```

Startup deterministically replays every history and fails closed on corruption. Checkpoints use atomic rename. Events contain `identifier_hash`, not a plaintext consumer email. Recovery exists in the enum but is disabled.

```text
event_hash = SHA256(UTF8("SatsPathNameEventV1") || canonical_json_utf8(event_without_owner_signature))
leaf_hash  = SHA256(0x00 || raw_32_byte_event_hash)
node_hash  = SHA256(0x01 || left_32 || right_32)
```

The owner signs the event hash under `SatsPathNameEventSignatureV1`. The exact profile and rotation encodings are in `protocol.md`.

## Resolution and proof verification

```mermaid
sequenceDiagram
  participant Client
  participant Resolver
  participant Log
  Client->>Resolver: identifier
  Resolver->>Log: profile + event + proofs + checkpoint
  Log-->>Client: provenance bundle
  Client->>Client: verify operator signature and pinned checkpoint
  Client->>Client: verify append-only consistency
  Client->>Client: replay identifier history/key continuity
  Client->>Client: verify inclusion, profile hash/signature and method ownership
```

Inclusion proofs are compact audit paths. V1 consistency proofs intentionally carry all new-tree leaf hashes: this is bandwidth-heavy but independently proves that the old root is the exact prefix and the new root is the full tree. Compact RFC6962 consistency paths are future work.

The resolver returns separate states for profile signature, identifier attestation, key continuity, inclusion, checkpoint consistency and payment-method ownership. Missing evidence never becomes a vague `verified: true`.

## Key rotation

```mermaid
flowchart LR
  K1[Old key K1] -->|AuthorizationV1| R[Rotation statement]
  K2[New key K2] -->|AcceptanceV1| R
  R --> E[RotateKey event signed by K1]
  K2 -->|signs new profile| P[New profile]
  E --> V{Verifier}
  P --> V
```

Both statements bind identifier hash, old/new keys, previous event hash, strictly increasing sequence and timestamp. Direct self-signed replacement is rejected. No emergency recovery is invented.

## Checkpoint consistency and split views

```mermaid
flowchart LR
  C1[Checkpoint N] -->|consistency proof| C2[Checkpoint N+k]
  P[Pinned operator/size/root/hash] --> D{Compare C2}
  D -->|smaller size| R[Critical rollback]
  D -->|same size, other root| S[Critical equivocation]
  D -->|invalid prefix| B[Critical inconsistency]
  D -->|valid| U[Atomic pin update]
```

The operator signs every checkpoint field, including an optional Bitcoin receipt. `checkpoint_hash` excludes the signature and receipt to avoid a txid/checkpoint circular commitment; the post-anchor signature commits to the receipt. Operator signatures create attributable evidence, but global gossip remains future work. First contact is TOFU unless independently compared.

## Identifier attestations

`IdentifierAttestationV1` binds identifier hash, identity key, profile hash, random nonce, issuance/expiry, method and verifier key. The verifier signs canonical JSON under `SatsPathIdentifierAttestationV1`. The current mock email challenge is not production verification; results report `identifier_verified: false` without a real attestation.

## Regtest anchoring

```mermaid
sequenceDiagram
  participant Daemon
  participant Core as Bitcoin Core regtest
  Daemon->>Daemon: SHA256("SatsPathCheckpointAnchorV1" || checkpoint_hash)
  Daemon->>Core: OP_RETURN transaction
  Daemon->>Core: fund, wallet-sign, broadcast, mine one block
  Daemon->>Core: fetch verbose transaction
  Core-->>Daemon: script + block + confirmations
  Daemon->>Daemon: verify commitment
```

Anchoring is never automatic. Authenticated `POST /v1/transparency/anchors` requires `SATSPATH_BITCOIN_NETWORK=regtest` plus `SATSPATH_BITCOIN_RPC_URL`, `SATSPATH_BITCOIN_RPC_USER` and `SATSPATH_BITCOIN_RPC_PASSWORD`. Mainnet is rejected and credentials are never logged. OP_RETURN provides public evidence but unrelated anchors do not fully prevent split views. Future Catena-style chaining would make transaction N spend a continuation output from N-1; it is not implemented.

## Explorer data flow

```mermaid
flowchart TB
  API[Paginated read-only APIs] --> D[Dashboard]
  API --> M[Bounded Merkle view]
  API --> H[Identifier history]
  API --> C[Checkpoint explorer]
  API --> P[Proof JSON]
  P --> W[Browser WebCrypto verification]
  W --> O[Hash steps + computed root]
```

The embedded daemon UI reuses the existing stack, limits event rendering, supports pan/zoom and leaf selection, highlights proof material and verifies pasted inclusion proofs locally. It sends no private material to the browser.

## Persistence and API

Events use append-only schema-v1 JSONL with `fsync`; checkpoints and local pins use temporary files plus atomic rename. The tree is rebuilt from events, making corruption visible. This is appropriate for a local first version; multi-process locking and a database migration remain hardening work.

Read APIs cover status, paginated events/checkpoints, identifier/event/checkpoint lookup, inclusion/consistency proofs, local proof verification and stored anchors. Pages are capped at 200 records. Registry mutation is not publicly exposed.

## Threats and limitations

- Initial registration still needs an external identity attestation.
- First contact is TOFU unless a checkpoint/key fingerprint is independently verified.
- A compromised verifier can attest a false binding.
- A compromised current key can update, revoke or authorize rotation; transparency makes this visible but cannot undo it.
- A lost key has no recovery in V1. Email recovery reduces security to the email provider.
- A malicious operator can attempt split views; pinning detects local rollback/conflict, while gossip is future work.
- Bitcoin provides auditable evidence; unrelated OP_RETURNs do not provide Catena's single-history property.
- An authenticated current-state/non-inclusion map and compact consistency proofs remain future work.
- This is experimental and unaudited. Do not use it with real funds.

Conceptual prior art: Zooko's Triangle, Certificate Transparency, CONIKS, Keybase, Catena and append-only authenticated data structures.
