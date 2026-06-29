# SatsPath

**A universal Bitcoin payment resolver and router.**

> DISCLAIMER: This is hackathon software. Do not use with real funds.

---

## Problem

Bitcoin has multiple payment rails — Lightning, on-chain, Ark, LNURL, BOLT12,
Silent Payments — each with different trade-offs on speed, cost, and privacy.
Users and apps must manually decide which rail to use, and there is no universal
way to discover payment methods from a human-readable identifier.

## Solution

SatsPath maps human-readable identifiers (e.g. `alice@example.com`) to **signed
payment profiles** containing all available payment methods. A routing engine
selects the best rail automatically based on amount, on-chain fees, and available
methods.

Private keys always remain with the user. SatsPath is a resolution and routing
layer — not a wallet, not a custodian.

---

## What this prototype does

- Resolve `alice@example.com` to a signed payment profile.
- Encode/decode universal payment URIs (`satspath:v1:<base64url_json>`).
- Sign profiles with secp256k1 identity keys.
- Verify profile signatures before routing.
- Select the best payment rail (Lightning → On-chain → Ark).
- Fetch real-time on-chain fee estimates from mempool.space.
- Simulate payment execution on the selected rail.
- Generate invite links for unregistered users (receiver generates their own keys).
- Full CLI: `init`, `register`, `show`, `encode`, `decode`, `quote`, `pay`, `invite`, `demo`.

## What this prototype does NOT do yet

- Execute real Lightning, on-chain, or Ark payments.
- Verify email or domain ownership during registration.
- Persist profiles to a decentralized registry (BIP-353, Nostr, DNS).
- Support key rotation or profile revocation.
- Implement BOLT12 invoice fetching.
- Support Silent Payments or Split Payments.
- Provide a wallet UI.

---

## Architecture

```
Human-readable identifier
  (e.g. rodrigo@satspath.dev)
          |
          v
  ┌───────────────────┐
  │  Resolver /       │   <-- local .satspath/registry.json (prototype)
  │  Registry         │   <-- BIP-353 / Nostr / DNS (future)
  └────────┬──────────┘
           |
           v
  ┌───────────────────┐
  │  Signed Payment   │
  │  Profile          │
  │  (secp256k1 sig)  │
  └────────┬──────────┘
           |
           v
  ┌───────────────────┐
  │  Route Engine     │
  └──────┬────┬───────┘
         |    |    \
         v    v     v
    ┌────┐ ┌──────┐ ┌─────┐
    │ LN │ │ BTC  │ │ Ark │
    └────┘ └──────┘ └─────┘
```

### Crate layout

```
satspath/
├── crates/
│   ├── satspath-core/     # Types, crypto, codec, registry
│   ├── satspath-router/   # Route selection engine + fee API
│   └── satspath-cli/      # CLI binary
├── examples/
│   ├── rodrigo_profile.json
│   └── demo_flow.md
└── docs/
    ├── architecture.md
    ├── threat_model.md
    └── protocol.md
```

---

## Installation

```bash
git clone https://github.com/<your-org>/satspath
cd satspath
cargo build --release
# Binary at: target/release/satspath
```

Or run directly:

```bash
cargo run -p satspath-cli -- <command>
```

---

## CLI Usage

### Initialize

```bash
satspath init
```

Creates `.satspath/registry.json` and `.satspath/keys.json` locally.
These directories are git-ignored.

### Register

```bash
satspath register rodrigo@satspath.dev
```

Generates a fresh secp256k1 identity keypair, builds a demo payment profile
with Lightning, on-chain (×2 for privacy), and Ark methods, signs the profile,
and stores it in the local registry.

### Show

```bash
satspath show rodrigo@satspath.dev
```

```
Alias:          rodrigo@satspath.dev
Identity pubkey:02a1b2c3...
Fingerprint:    a1b2c3d4
Signature valid: yes
Updated at:     1735000000

Methods:
  - Lightning Address [Lightning]
      Lightning Address: rodrigo@satspath.dev
  - Bitcoin (primary) [On-chain]
      Address: bc1q...
  - Bitcoin (secondary) [On-chain]
      Address: bc1q...
  - Ark [Ark]
      Server: ark.satspath.dev
```

### Encode a payment URI

```bash
satspath encode rodrigo@satspath.dev 21000 --memo "coffee"
```

```
satspath:v1:eyJ2ZXJzaW9uIjox...
```

### Decode a payment URI

```bash
satspath decode "satspath:v1:eyJ2ZXJzaW9uIjox..."
```

```
Decoded payment request:
  Version:     1
  Alias:       rodrigo@satspath.dev
  Amount:      21000 sats
  Memo:        coffee
  Profile hint:(none)
```

### Quote (route selection)

```bash
satspath quote rodrigo@satspath.dev 21000
```

```
Resolving alias...
Verifying signature...
Checking payment rails for 21000 sats...

Route Quote:
  Selected rail:   Lightning
  Label:           Lightning Address
  Reason:          Amount (21000 sats) is below 100000 sats threshold and Lightning is available.
  Estimated fee:   2 sats
  Confirmation:    instant
```

### Pay (simulated)

```bash
satspath pay rodrigo@satspath.dev 21000
```

```
Resolving alias 'rodrigo@satspath.dev'...
Verifying signed profile...
Checking available payment rails...
Selected route: Lightning
Reason:         Amount (21000 sats) is below 100000 sats threshold and Lightning is available.

Payment status: simulated_success
```

### Invite (unregistered user)

```bash
satspath invite julian@example.com 21000
```

```
'julian@example.com' is not registered on SatsPath.

Invite link:
https://satspath.local/claim?alias_hash=a1b2c3d4...&amount=21000

WARNING: The receiver must claim this payment by generating their own keys locally.
```

### Full demo

```bash
satspath demo
```

Runs all steps above automatically.

---

## Example demo flow

See [examples/demo_flow.md](examples/demo_flow.md) for a full annotated walkthrough.

---

## Security model

- **Keys are local.** Private keys are generated and stored only on the user's machine
  in `.satspath/keys.json`, which is git-ignored and never transmitted.
- **Signatures are mandatory.** Every payment profile is signed with the owner's
  identity key. Signature verification happens before routing.
- **Public profiles only.** The registry stores only public data: pubkeys, addresses,
  and signatures.
- **Invite, don't proxy.** For unknown users, SatsPath creates an invite. The receiver
  generates their own keys.

---

## Threat model summary

| Threat | MVP Mitigation |
|--------|---------------|
| Fake alias registration | Signature-bound profiles; first-come-first-served |
| Server tampering | secp256k1 signature verification on every resolve |
| Key replacement attack | Signature covers identity_pubkey; swap breaks sig |
| Email takeover | Not mitigated at MVP (future: DKIM challenge) |
| LNURL spoofing | Not mitigated at MVP (future: TLS pinning) |
| On-chain privacy leaks | Multiple on-chain addresses per profile |
| Lost keys | Local backup only (future: BIP-39 seed phrase) |
| Malicious invite links | Alias hash + amount in invite; receiver verifies |
| Ark server trust | Mock client only (future: covenant-based Ark) |

Full threat model: [docs/threat_model.md](docs/threat_model.md)

---

## Future integrations

| Integration | Purpose |
|-------------|---------|
| **BIP-353** | Replace local registry with DNS TXT record resolution |
| **Nostr** | Decentralized identity and profile discovery (NIP-05, NIP-57) |
| **Lightning Address** | Auto-discover LNURL-pay endpoint from `user@domain` |
| **BOLT12** | Async invoices and reusable payment offers |
| **Ark** | Non-custodial off-chain payments via virtual UTXOs |
| **Silent Payments** | Privacy-preserving on-chain payments (BIP-352) |
| **Split Payments** | Route a single payment across multiple rails |

---

## Running tests

```bash
cargo test
```

Test coverage includes:
- Profile serialization
- Profile signing and verification
- Invalid signature rejection
- Tampered profile rejection
- Encode/decode payment URIs
- Registry register/resolve
- Router: Lightning for small amounts
- Router: On-chain for low fees
- Router: Ark fallback for high fees
- Router: Error when no methods available
- Registry persistence across opens

---

## Hackathon scope

This prototype was built during a hackathon to demonstrate:

1. A working secp256k1-signed payment profile format.
2. A universal URI encoding scheme for payment requests.
3. An automatic payment rail selector using real mempool.space fee data.
4. A privacy-aware profile with multiple on-chain addresses.
5. An invite flow that preserves receiver key sovereignty.

It is **not production-ready**. The registry is local, payments are simulated,
and domain verification is not implemented.

---

## DISCLAIMER

This is hackathon software. Do not use with real funds.
