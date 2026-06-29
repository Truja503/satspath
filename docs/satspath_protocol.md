# SatsPath Protocol

SatsPath is an open Bitcoin payment resolver and routing protocol. Wallets can
implement it without using a SatsPath-hosted platform.

The protocol flow is:

```txt
identifier -> signed public profile -> public payment pointers -> route decision -> QR/payment intent
```

## Public Profile

A `PaymentProfile` contains public information only:

- identifier alias,
- identity public key,
- Lightning Address, LNURL, or BOLT-style public invoice data,
- on-chain public addresses and public descriptor hints,
- Ark server and receiver public key,
- update and expiry timestamps.

The profile is signed by the user's identity key. That identity key does not
control funds by itself. Wallets and nodes control funds.

## Payment Pointers

`PaymentPointer` is the stable output from resolution/routing. It may contain:

- Lightning Address,
- LNURL-pay callback,
- BOLT11 invoice as data,
- Bitcoin address with public claim policy metadata,
- Ark public pointer.

It must never contain private keys, seed phrases, macaroons, certificates, API
secrets, or signing keys.

## Platform

The SatsPath Platform is optional. It can provide:

- email verification for normal users,
- invite links,
- API-based profile publishing,
- public profile lookup.

It must never custody funds and must never see wallet private keys.

## Compatibility

BIP-353 discovers payment instructions through DNS. BIP-321/BIP-21 style
`bitcoin:` URIs encode payment instructions. SatsPath extends this by adding
signed public profiles and route selection across Lightning, on-chain Bitcoin,
and Ark.
