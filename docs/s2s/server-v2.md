# SatsPath v2 Authoritative Server

## Overview

The SatsPath v2 server is an authoritative, network-facing identity resolution service that hosts one or more namespaces and returns fully proof-carrying resolution envelopes. It is strictly a discovery layer: it does not sign or broadcast payments and accepts no wallet spending secrets.

## V2 API Endpoints

### `GET /v2/namespace`

Returns the signed `NamespaceDescriptor` for the hosted domain, including the operator public key, witness quorum policy, and endpoint list.

### `GET /v2/resolve/:identifier`

Returns a complete `ResolutionEnvelope` containing:

- The signed payment profile.
- All name events for the identifier.
- Merkle inclusion proof binding the event to the log.
- Consistency proof (if the client provides a pinned tree size).
- Current-state map proof (inclusion or non-inclusion).
- Witness cosignatures meeting the quorum threshold.
- The latest signed checkpoint.

### `GET /v2/checkpoint/latest`

Returns the latest signed `TransparencyCheckpoint` with its witness cosignatures.

### `GET /v2/health`

Returns readiness status, version, checkpoint age, witness quorum health, and replica lag metrics.

## Resolution Pipeline

```
canonical identifier
-> discover authoritative namespace
-> fetch proof envelope
-> verify namespace binding
-> verify owner event/profile
-> verify current-state proof
-> verify append-only inclusion + consistency
-> verify operator continuity + witness policy
-> verify method ownership
-> route
```

Every quote/pay/preview path must consume the same verified result. Legacy data or transport responses must not bypass this composition.

## Security Requirements

- HTTPS required in deployment; strict origin/redirect rules in the client.
- Atomic SQLite persistence remains fail-closed under crashes.
- Bounded request/response bodies, proof depths, histories, concurrency, and timeouts.
- Rate limits per namespace/IP without changing cryptographic truth.
- SSRF prevention through descriptors, payment-method metadata, redirects, and replica URLs.
- No logging of auth tokens, challenges, full sensitive request metadata, or any private keys.
- Multi-tenant namespace separation prevents cross-namespace reads, writes, proof reuse, and key confusion.
