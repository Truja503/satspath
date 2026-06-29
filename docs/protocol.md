# SatsPath Protocol Specification (v1 — Prototype)

## Overview

SatsPath is a **resolution and routing layer** for Bitcoin payments.
It is not a wallet, not a node, and not a replacement for Lightning, Ark, or BIP-353.
It is the glue that maps human-readable identifiers to signed payment profiles
and selects the best available payment rail.

## Identifier Format

SatsPath uses email-style identifiers:

```
<local-part>@<domain>
```

Examples:
- `rodrigo@satspath.dev`
- `alice@example.com`
- `shop@btcstore.io`

The identifier is case-insensitive in the local part (normalized to lowercase).

## Universal URI Format

### Simple form

```
satspath:<alias>
```

Example:
```
satspath:rodrigo@satspath.dev
```

Used for QR codes, NFC, and simple payment links where amount is negotiated
out-of-band.

### Encoded form

```
satspath:v1:<base64url_no_pad_json>
```

The payload is a base64url-encoded (no padding) JSON object:

```json
{
  "version": 1,
  "alias": "rodrigo@satspath.dev",
  "amount_sats": 21000,
  "memo": "coffee payment",
  "profile_hint": null
}
```

Field definitions:
- `version`: Protocol version. Currently `1`.
- `alias`: The payment recipient identifier.
- `amount_sats`: Optional requested amount in satoshis.
- `memo`: Optional human-readable payment description.
- `profile_hint`: Optional pre-fetched profile data for offline scenarios.

## Signed Payment Profiles

A `PaymentProfile` contains the public, shareable information about a user:

```json
{
  "alias": "rodrigo@satspath.dev",
  "identity_pubkey": "<hex-encoded secp256k1 compressed pubkey>",
  "methods": [ ... ],
  "updated_at": 1735000000,
  "expires_at": 1736000000
}
```

A `SignedPaymentProfile` wraps the profile with an ECDSA signature:

```json
{
  "profile": { ... },
  "signature": "<hex-encoded DER ECDSA signature>"
}
```

### Signing

The signature is computed as:

```
signature = ECDSA.sign(
  key = identity_secret_key,
  message = SHA256(canonical_json(profile))
)
```

Where `canonical_json` is the deterministic JSON serialization of the profile
(field order preserved via Rust's derived `Serialize`).

### Verification

Before processing any payment, the receiver's profile MUST be verified:

1. Parse `identity_pubkey` as a secp256k1 compressed public key.
2. Compute `SHA256(canonical_json(profile))`.
4. Check the `expires_at` timestamp (if present). If it is strictly in the past, reject with `ExpiredProfile`.
5. If verification fails, reject with `InvalidSignature`.

This validation MUST happen on both local registry lookups and remote HTTP fetches.

## Payment Methods

### Lightning

```json
{
  "type": "Lightning",
  "label": "Lightning Address",
  "lnurl": null,
  "lightning_address": "rodrigo@satspath.dev",
  "bolt12": null
}
```

At least one of `lnurl`, `lightning_address`, or `bolt12` must be non-null
for the Lightning method to be considered available.

### On-chain

```json
{
  "type": "Onchain",
  "label": "Bitcoin (primary)",
  "address": "bc1q...",
  "pubkey_hint": "02a1b2..."
}
```

Multiple on-chain methods are encouraged for privacy. Each should use a
distinct address and pubkey hint.

### Ark

```json
{
  "type": "Ark",
  "label": "Ark",
  "server": "ark.satspath.dev",
  "pubkey": "02a1b2..."
}
```

## Routing Algorithm

```
Input: alias, amount_sats, signed_profile

1. Verify signature (fail if invalid)

2. If amount_sats < 100,000 sats:
     Find first Lightning method with lnurl OR lightning_address OR bolt12
     If found → select Lightning

3. Fetch hourFee from mempool.space/api/v1/fees/recommended
   (fallback: hourFee = 5 on API error)
   If hourFee ≤ 10 sat/vB AND on-chain address exists:
     → select On-chain

4. If Ark method exists:
     → select Ark

5. Otherwise → error: no suitable rail
```

## Invite Flow

When the resolver cannot find a registered alias, the sender creates an invite
instead of failing silently:

```
invite = {
  alias_hash: SHA256(alias)[0..32 hex chars],
  amount_sats: <requested amount>,
  created_at: <unix timestamp>,
  claim_url: "https://satspath.local/claim?alias_hash=...&amount=...",
  warning: "Receiver must generate their own keys locally."
}
```

The invite is shared out-of-band (QR, link, message). The receiver:

1. Generates a keypair locally.
2. Registers their alias with their public key.
3. The sender retries the payment.

SatsPath never generates or holds keys on behalf of the receiver.

## Why SatsPath is NOT a Replacement

| Protocol | Role | SatsPath relationship |
|----------|------|-----------------------|
| Lightning | Fast, cheap payments | SatsPath routes to Lightning when appropriate |
| Ark | Non-custodial off-chain | SatsPath routes to Ark as fallback |
| BIP-353 | DNS-based resolution | SatsPath registry is a prototype; production would use BIP-353 |
| BOLT12 | Async invoices | SatsPath stores BOLT12 offers in the profile |
| Silent Payments | Privacy-preserving on-chain | Future on-chain method type |
| Nostr | Decentralized identity | Future registry backend |
| LNURL | Lightning metadata protocol | SatsPath stores LNURL in Lightning method |

SatsPath is the **routing and resolution layer**, not the payment layer.
It answers: "Given this identifier and amount, which rail should I use?"
The actual payment execution happens in the underlying protocol.

## Future Work

- Replace local registry with BIP-353 DNS TXT record lookup.
- Support Nostr NIP-05 profile resolution.
- Add BOLT12 offer fetching and invoice generation.
- Add Silent Payments as an on-chain method type.
- Split payments across multiple rails.
- Implement key rotation with signature chain.
