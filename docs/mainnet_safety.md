# Mainnet Safety & Security Guidelines

SatsPath is a powerful routing engine that coordinates interactions across multiple Bitcoin layers (Lightning, On-chain, and Ark). Moving real funds on Mainnet carries inherent risks. This document establishes the security perimeter for the engine.

## 1. Non-Custodial Architecture

**SatsPath is NOT a custodial wallet.**
- The `Identity Key` (used to sign profiles) **does not** control funds.
- Wallet, Node, or SDK plugins are responsible for securely managing funds and signing transactions.
- Never store seed phrases, wallet private keys, macaroons, certs, API tokens, or other high-value secrets in the SatsPath repository or plaintext config files.
- Email verification proves inbox access only. It does not transfer custody and
  does not prove ownership of a Gmail-style domain.
- Receiver wallets must generate private keys locally on the receiver's device
  and publish only public payment profiles.

## 2. Mainnet Configuration

By default, the SatsPath Swap Engine operates in **Testnet mode**.

**Required Safety Defaults:**
- `mainnet_enabled = false`
- `max_mainnet_payment_sats = 1000`
- `require_manual_confirmation = true`
- `fail_closed = true`

Mainnet preview is allowed because it touches public data only. Mainnet
execution is disabled. `--experimental-swaps --testnet` is for testnet-only
engine scaffolding; mainnet execution requires a separate future feature with
stronger confirmation gates.

## 3. Strict Pre-Execution Checks

Before executing *any* Mainnet transaction or Swap, the engine MUST abort if any of the following checks fail:
- **Amount Mismatch:** Abort if the requested invoice amount does not match the BOLT11 invoice returned by LNURL or Boltz.
- **Signature Verification:** Abort if the `SignedPaymentProfile` signature is invalid or tampered with.
- **Metadata Invalid:** Abort if LNURL metadata violates expected tags or amount bounds.
- **Expiration:** Abort if the payment profile has expired.

## 4. First Mainnet Tests

When testing features on Mainnet for the first time:
- Use tiny amounts only (e.g., `< 1000 sats`).
- Verify routing paths locally before broadcasting.
- Ensure that the local `.satspath/swaps.enc` vault is encrypting secrets via AES-GCM and not falling back silently to plaintext.

## 5. BIP-353 DNS Resolution (Mainnet Preview)

BIP-353 resolution is a **preview** layer: SatsPath resolves and displays
DNSSEC-backed payment instructions but never pays, signs, or broadcasts.

- **DNSSEC is mandatory.** The default `Strict` policy fails closed and does not
  trust an upstream resolver's AD bit. `DevInsecure` mode is local-testing only,
  requires `--allow-insecure-dns-for-dev`, and prints a loud warning.
- **Ambiguity is invalid.** More than one `bitcoin:` TXT record at a name, or an
  unknown `req-*` parameter, makes resolution fail.
- **No private material** may ever appear in a published or resolved DNS payload
  (`seed`, `xprv`/`tprv`, `mnemonic`, `macaroon`, `cert`, `api_key`, `claim_key`,
  `refund_key`, `preimage`, …) — screened on both publish and resolve.
- **Record changes require an identity-key signature.** Email access alone never
  authorizes a DNS payment-instruction change.
- **Consumer email domains** (e.g. `gmail.com`) cannot use BIP-353; they fall back
  to platform verification / the invite flow.
- **DNS-provider credentials are never committed** — only a trait + mock publisher
  ship in this repo.
