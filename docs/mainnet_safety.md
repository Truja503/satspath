# Mainnet Safety & Security Guidelines

SatsPath is a powerful routing engine that coordinates interactions across multiple Bitcoin layers (Lightning, On-chain, and Ark). Moving real funds on Mainnet carries inherent risks. This document establishes the security perimeter for the engine.

## 1. Non-Custodial Architecture

**SatsPath is NOT a custodial wallet.**
- The `Identity Key` (used to sign profiles) **does not** control funds.
- Wallet, Node, or SDK plugins are responsible for securely managing funds and signing transactions.
- Never store seed phrases, wallet private keys, macaroons, certs, API tokens, or other high-value secrets in the SatsPath repository or plaintext config files.

## 2. Mainnet Configuration

By default, the SatsPath Swap Engine operates in **Testnet mode**.

**Required Safety Defaults:**
- `mainnet_enabled = false`
- `max_mainnet_payment_sats = 1000`
- `require_manual_confirmation = true`
- `fail_closed = true`

Mainnet execution requires explicit confirmation via the CLI (`--experimental-swaps --testnet` are for testing only; mainnet will require a different explicit flag once enabled).

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
