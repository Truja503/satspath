# SatsPath

**Self-Sovereign Bitcoin Payment Resolution & Multi-Rail Smart Routing Protocol (v2)**

> **CAUTION:**
> **EXPERIMENTAL SOFTWARE — DO NOT USE WITH REAL FUNDS.**
> SatsPath v2 is currently undergoing internal conformance testing (Issue #60). It must not be deployed for real economic activity until the v2 conformance suite is finalized and an external cryptographic audit gate is deliberately cleared.

---

## 🧭 What is SatsPath?

**SatsPath** is an open-source, non-custodial protocol that maps human-readable identifiers (e.g., `alice@domain.com` or `chelo@satspath.dev`) to **cryptographically signed payment profiles** across all major Bitcoin payment rails:

- **⚡ Lightning Network:** Instant micro-payments via BOLT11 invoices, BOLT12 offers, and LNURL-pay.
- **🏹 Ark Protocol:** Layer-2 payment URI preview and intent generation via virtual UTXOs (VTXOs) (execution preview-only).
- **⛓️ Bitcoin On-Chain:** Standard BIP-21 addresses and privacy-preserving Silent Payments (BIP-352).

SatsPath acts as a **zero-trust cryptographic GPS for Bitcoin payments**: it never holds custody of funds, never generates or accesses wallet spending keys (`xprv`/`tprv`), and never broadcasts financial transactions.

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│ 1. Identity Key Derivation (BIP-39 Seed -> m/9737'/0' via HMAC-SHA512)     │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 2. S2S v2 Resolution & Client-Side Verification                             │
│    • S2S v2 HTTPS / DNSSEC (BIP-353) / Nostr (NIP-05)                       │
│    • secp256k1 Schnorr Signatures & Monotonic Sequence Validation           │
│    • RFC 6962 Append-Only Merkle Transparency Logs + Witness Quorum         │
│    • Bitcoin On-Chain Anchoring (OP_RETURN checkpoints)                     │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 3. Multi-Rail Smart Routing (satspath-router)                               │
│    • Amount threshold scoring + Live mempool fee evaluation                 │
│    • Automatic rail selection: Lightning (< 100k sats) | Ark | On-Chain     │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 4. Standard Payment Handoff (BOLT11/12 QR, Ark URI, BIP-21 URI)            │
│    • Public payload handed to host wallet for native execution              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 📦 Workspace Architecture & Crates

The SatsPath repository is structured as a modular Rust workspace and TypeScript SDK:

| Crate / Package | Target | Description |
| :--- | :--- | :--- |
| **`crates/satspath-core`** | Rust / Native / FFI | Core protocol types, `secp256k1` Schnorr crypto, RFC 6962 Merkle transparency engine, SSRF protections, and resolver chain. |
| **`crates/satspath-router`** | Rust / WASM | Intelligent multi-rail routing engine, live `mempool.space` fee evaluator, and Silent Payments support. |
| **`crates/satspath-wasm`** | WASM / JS / TS | Lightweight WebAssembly bindings for Web, PWAs, and React Native mobile wallets. |
| **`crates/satspath-cli`** | Rust Binary | Sovereign developer and node operator CLI (`satspath`). |
| **`crates/satspathd`** | Server Daemon | Authoritative S2S v2 server with Bitcoin on-chain checkpoint anchoring and REST API (`/v2/resolve`, `/v2/namespace`). |
| **`crates/satspath-witness`** | Server Daemon | Independent transparency witness auditing split-view attacks. |
| **`crates/satspath-pqc`** | Rust Library | Post-quantum hybrid cryptography primitives (ML-KEM / Falcon experimental). |
| **`sdk/`** | TypeScript (`pnpm`) | High-level TypeScript SDK packages (`@satspath/resolvers`, `@satspath/router`). |

---

## ⚡ Quickstart for Wallet Developers (`pnpm`)

Integrate SatsPath resolution into your web app, PWA, or wallet frontend in minutes:

### 1. Installation

```bash
pnpm add @satspath/wasm bip39
# or modular TS packages
pnpm add @satspath/resolvers @satspath/router
```

For full integration instructions, see [docs/SDK_QUICKSTART.md](docs/SDK_QUICKSTART.md) or use the [AI Vibe-Coding Master Prompt](docs/VIBECODE_INTEGRATION_PROMPT.md) for Cursor/Claude.

### 2. Basic TypeScript Integration

```typescript
import init, { derive_identity_keypair_from_seed, quote } from '@satspath/wasm';
import * as bip39 from 'bip39';

// 1. Initialize WASM module
await init();

// 2. Deterministically derive SatsPath identity inside the trusted wallet boundary
// Uses HMAC-SHA512 with domain separator "SatsPath Identity Key m/9737'/0'" and big-endian account index.
// Seed/mnemonic remains private inside the wallet context and is never transmitted.
const walletSeedMnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const seed = bip39.mnemonicToSeedSync(walletSeedMnemonic);
const identity = derive_identity_keypair_from_seed(seed, 0);
if (identity) {
  console.log("SatsPath Identity Pubkey:", identity.pubkey_hex);
}

// 3. Resolve alias and quote the best payment route
const quoteResult = await quote("chelo@satspath.dev", 25000n);

if (quoteResult.status === "ok") {
  console.log("Selected Rail:", quoteResult.selected_method.type); // "Lightning" | "Ark" | "Onchain"
  console.log("Estimated Fee:", quoteResult.fee_sats, "sats");
  console.log("Payment Payload:", quoteResult.qr); // Standard QR/URI for wallet execution
}
```

---

## 🛠️ CLI Usage (`satspath`)

### Installation & Build

```bash
git clone https://github.com/marchelo23/satspath.git
cd satspath
cargo build --release
# Binary available at target/release/satspath
```

### Core Commands

#### 1. Register a Profile

```bash
satspath register chelo@satspath.dev --testnet
```

Generates identity keys, derives public addresses (Lightning, On-Chain, Ark), signs the profile with Schnorr `secp256k1`, and records it in the local transparency log.

#### 2. Quote & Smart Route

```bash
# Micro-payment (< 100k sats) -> Routes to Lightning
satspath quote chelo@satspath.dev 21000 --testnet

# Large settlement (500k sats) -> Routes to On-Chain L1 with live fee calculation
satspath quote chelo@satspath.dev 500000 --testnet
```

#### 3. Encode & Decode Universal Payment URIs

```bash
# Encode payment request
satspath encode chelo@satspath.dev 50000 --memo "SatsPath MVP"

# Decode payment URI
satspath decode "satspath:v1:eyJ2ZXJzaW9uIjoxLCJhbGlhcyI6ImNoZWxvQHNhdHNwYXRoLmRldiIsImFtb3VudCI6NTAwMDB9"
```

#### 4. Authoritative Server Daemon (`satspathd`)

Run the S2S v2 daemon locally or in Docker:

```bash
docker-compose up -d satspathd
```

Exposes:
- `GET /.well-known/satspath-authority` — Signed namespace descriptor
- `GET /v2/resolve?identifier=user@domain` / `POST /v2/resolve` — Proof-carrying `ResolutionEnvelope`
- `GET /v2/health` — Checkpoint age and witness quorum health

---

## 🔒 Security & Trust Model

SatsPath v2 is built under strict **zero-trust, fail-closed security principles**:

1. **Non-Custodial Architecture:** Identity keys are derived under BIP-32 path `m/9737'/0'` specifically isolated from Bitcoin wallet spending paths (`m/84'/0'/0'`, `m/86'/0'/0'`).
2. **Cryptographic Identity Binding:** Every profile is signed with Schnorr `secp256k1`. Senders cryptographically verify that `canonical_identifier(requested) == profile.alias` before displaying or paying.
3. **Tamper-Proof Merkle Transparency:** Mutations are appended to RFC 6962 Merkle trees. Nodes verify inclusion proofs against signed checkpoints anchored to the Bitcoin blockchain (`OP_RETURN`).
4. **SSRF & Network Hardening:** All outbound resolver requests enforce strict DNS/IP validation, private/loopback IP blocking, port allowlisting, and trailing-dot normalization.
5. **Universal Fallback (BIP-353):** Full backward compatibility with standard DNSSEC payment instructions.

---

## 📚 Documentation Index

- [⚡ SDK Quickstart (`pnpm`)](docs/SDK_QUICKSTART.md) — Fast integration guide for frontend/wallet devs.
- [🤖 AI Vibe-Coding Integration Prompt](docs/VIBECODE_INTEGRATION_PROMPT.md) — Plug-and-play prompt for Cursor/Claude/Copilot.
- [📐 Protocol Specification v2](docs/protocol.md) — Wire format, profile structures, and state transitions.
- [🌐 S2S Protocol & Server v2](docs/s2s/server-v2.md) — Authoritative server and namespace endpoints.
- [🛡️ Trust Model & Cryptographic Guarantees](docs/s2s/trust-model-v2.md) — Merkle proofs, witness quorums, and split-view prevention.
- [🌳 Key Transparency & Bitcoin Anchoring](docs/key_transparency.md) — RFC 6962 log design and `OP_RETURN` anchor flow.
- [🐳 Docker Deployment Guide](docs/docker.md) — Production container orchestration and security profiles.
- [🔒 Mainnet Safety Policies](docs/mainnet_safety.md) — Security boundaries and non-custodial invariants.

---

## 🧪 Testing & Continuous Integration

Run the comprehensive test suite locally:

```bash
# Run all workspace unit & integration tests
cargo test --workspace --all-features

# Run formatting & linters
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

All pull requests are automatically validated on GitHub Actions with:
- `CI / Test workspace` (Rust stable workspace tests & linters)
- `CI / Bitcoin Core regtest anchor` (Live Bitcoin node OP_RETURN anchoring)
- `Codacy Security Scan` (Static security analysis)
- `Docker Build & Scan` (Container image vulnerability scanning)

---

## 📄 License

Licensed under the MIT License or Apache-2.0 License at your option.
