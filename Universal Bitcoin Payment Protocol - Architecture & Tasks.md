# Universal Bitcoin Payment Protocol - Arquitectura y Tareas

A continuación, se resuelven las preguntas clave sobre la arquitectura de identidad y resolución, junto con la tabla de mitigación de riesgos y la lista de tareas (To-Do List) para la validación del lado del cliente de ARK.

## Preguntas de Arquitectura

**¿Quién firma el profile?**
- **MVP:** El servidor/servicio centralizado actúa como registro y **firma el profile** tras validar control del email.
- **Futuro:** El usuario firma su propio profile con su clave privada (**Self-Sovereign Identity**, estilo Nostr). El servidor solo distribuye el profile firmado.

> **Estado actual en satspath:** El usuario genera un keypair secp256k1 local (`.satspath/keys.json`) y firma su propio profile. El servidor es solo un registry local — las claves privadas nunca salen del dispositivo.

---

**¿Qué pasa si cambia la llave?**
- **MVP:** El profile anterior se invalida/sobrescribe, requiriendo nueva validación de email/cuenta.
- **Futuro:** Historial de rotación de llaves (**Key rotation history**). Si una llave cambia repentinamente, se muestra advertencia al remitente (modelo **TOFU — Trust On First Use**) mostrando el fingerprint de la llave.

> **Estado actual en satspath:** `satspath show <alias>` muestra `Fingerprint: a1b2c3d4`. No hay key rotation implementado todavía (ver README: `Key rotation or profile revocation` como item NOT done).

---

**¿Cómo detectamos perfil falso?**
- **MVP:** **Email challenge** (código/enlace mágico) para confirmar acceso al email. Perfiles sin validar se marcan como "no verificados" y el cliente no los resuelve.
- **Futuro:** Web of Trust (Nostr), validación criptográfica atada a DNS (tipo NIP-05), o DIDs (Decentralized Identifiers).

> **Estado actual en satspath:** El registry es first-come-first-served sin email challenge. La signature verifica que el profile no fue alterado, pero no verifica ownership del email. `Email takeover: Not mitigated at MVP (future: DKIM challenge)` (ver threat_model del README).

---

**¿Qué pasa si alguien registra rodrigo@gmail.com sin ser dueño?**
- **MVP:** Al registrar, el sistema envía email a `rodrigo@gmail.com`. Sin acceso al correo, el atacante no puede confirmar. El profile queda pendiente/no verificado y el cliente no lo resuelve.
- **Futuro:** Verificación criptográfica via firmas de dominio (DNS) o identidad federada (OIDC) para prevenir secuestro total de alias.

> **Estado actual en satspath:** No hay protección contra esto en el MVP. La signature está atada al keypair, no al email. Registrar `rodrigo@gmail.com` con una clave falsa produce un profile firmado con la clave del atacante.

---

**¿Qué información se guarda localmente?**
- **En el cliente (wallet/app):**
  - Caché de profiles resueltos y verificados.
  - *Fingerprints* de llaves públicas de contactos (para detectar ataques de reemplazo de llave).
  - Preferencias de enrutamiento (prioridad de red, límites de fees).
  - Historial de pagos.
- **En ARK SDK (local storage cifrado AES-256-GCM):**
  - Datos de **salida soberana**: secuencia de transacciones broadcast-ready ordenada `Anchor TX → … → VTXO Root`.
  - Llave maestra derivada via **PBKDF2** (100k iteraciones, SHA-256) desde password del usuario.
  - Sin esta data, el usuario necesita al ASP para salir del protocolo ARK.

> **Estado actual en satspath:** Guarda localmente `.satspath/registry.json` (profiles públicos) y `.satspath/keys.json` (privkeys, git-ignored). Sin cifrado en reposo todavía.

---

**¿Qué NO podemos prometer?**
- **Resistencia total a censura** si dependemos de un servidor central de resolución en el MVP (el servidor podría negarse a resolver un email).
- **Privacidad absoluta de metadatos** de quién paga a quién si la consulta no es ofuscada (el registry sabe quién resuelve qué alias).
- **Evitar fees** subyacentes de la red seleccionada (on-chain, Lightning, Ark).

---

## Tabla de Amenazas y Mitigación

| Threat | Risk | MVP mitigation | Future mitigation |
| :--- | :--- | :--- | :--- |
| Fake email registration | High | mark as unverified | email challenge / DNS / Nostr verification |
| Key replacement attack | Medium | show key fingerprint | key history + warnings |
| Server tampering | High | signed profiles | decentralized registry |

---

## SatsPath — Código del Protocolo

**Repositorio clonado:** `/home/chelo/antigravity/satspath`  
**Fuente:** `https://github.com/Truja503/satspath`  
**Stack:** Rust (workspace con 3 crates)

### Estructura del proyecto

```
satspath/
├── crates/
│   ├── satspath-core/      # Tipos, crypto (secp256k1), codec, registry
│   │   ├── profile.rs      # PaymentProfile + firma
│   │   ├── crypto.rs       # Keypair gen, sign, verify (secp256k1)
│   │   ├── codec.rs        # Encode/decode URI satspath:v1:<base64url>
│   │   ├── registry.rs     # Local JSON registry (resolver)
│   │   └── errors.rs
│   ├── satspath-router/    # Motor de routing + fee API
│   │   ├── router.rs       # Rail selection: Lightning → On-chain → Ark
│   │   ├── fees.rs         # mempool.space fee estimation (real HTTP)
│   │   ├── lightning.rs
│   │   ├── onchain.rs
│   │   └── ark.rs
│   └── satspath-cli/       # CLI binary
│       └── commands/
│           ├── init.rs     # Genera registry + keys localmente
│           ├── register.rs # Keypair + profile + firma + guarda en registry
│           ├── show.rs     # Muestra alias, pubkey, fingerprint, sig valid
│           ├── encode.rs   # URI satspath:v1:<b64>
│           ├── pay.rs      # Resolve + verify sig + route + simulate pay
│           ├── quote.rs    # Resolve + route selection con razón
│           ├── invite.rs   # Invite link para alias no registrado
│           └── demo.rs     # Corre todo el flujo automáticamente
├── examples/
│   ├── rodrigo_profile.json
│   └── demo_flow.md
└── docs/
    ├── architecture.md
    ├── threat_model.md
    └── protocol.md
```

### Lo que ya funciona (MVP)

- ✅ Resolución `alice@example.com` → signed payment profile
- ✅ Firma de profiles con secp256k1 (usuario firma localmente)
- ✅ Verificación de firma antes de cualquier routing
- ✅ Encoding/decoding de URIs universales `satspath:v1:<base64url_json>`
- ✅ Selección automática de rail: **Lightning → On-chain → Ark** (según amount y fees)
- ✅ Fetch de fees reales desde **mempool.space**
- ✅ Simulación de pago en el rail seleccionado
- ✅ Invite links para usuarios no registrados (receiver genera sus propias keys)
- ✅ CLI completa: `init`, `register`, `show`, `encode`, `decode`, `quote`, `pay`, `invite`, `demo`

### Lo que NO está implementado aún

- ❌ Pagos Lightning/on-chain/Ark reales (todo simulado)
- ❌ Email challenge / verificación de ownership
- ❌ Registry descentralizado (BIP-353, Nostr, DNS)
- ❌ Key rotation / revocación de profile
- ❌ BOLT12 invoice fetching
- ❌ Silent Payments (BIP-352)

---

## To-Do List: ARK Client-Side Validation

> **Estado:** ✅ **104/104 tests pasan.** Toda la lógica de validación del lado del cliente está implementada.

Basado en el código auditado en `/home/chelo/antigravity/ARK/src/`:

- [x] **`cryptoUtils.ts` — Base Criptográfica**
  - [x] AES-256-GCM authenticated encryption para storage at rest.
  - [x] Derivación de llave de storage con SHA256.
  - [ ] *(Pendiente)* SHA256 / Schnorr helpers como exports independientes.

- [x] **`signatureVerification.ts` — Validación de Firmas Schnorr/MuSig2**
  - [x] `verifyDAGSignatures()` — traversal iterativo por todo el DAG.
  - [x] Verificación Schnorr BIP 340 contra tweaked public key (BIP 341).
  - [x] Soporte de `tapKeySig` 64 y 65 bytes con validación de sighash.
  - [ ] *(Pendiente)* Script-path spend verification (actualmente lanza `UNSUPPORTED_SPEND_PATH`).

- [x] **`timelockVerification.ts` — Validación de Timelocks**
  - [x] Extracción de `nLockTime`, `nSequence`, `OP_CSV`, `OP_CLTV` desde tapscripts.
  - [x] Validación de consistencia interna (4 reglas BIP 65/68/112).
  - [x] Satisfiability checks contra estado actual de la cadena.
  - [x] `verifyDAGTimelocks()` — traversal iterativo por todo el DAG.

- [x] **`taprootVerification.ts` — Validación Taproot**
  - [x] Consistencia `tapInternalKey` ↔ `tapMerkleRoot` ↔ `witnessUtxo.script`.
  - [x] Verificación de Merkle proof (control block raw y decodificado).
  - [x] `verifyArkExitPolicy()` — ARK estándar (CSV+CHECKSIG), HTLCs, refunds.
  - [x] Rechazo de scripts triviales OP_TRUE (SECURITY_VIOLATION).

- [x] **`hashPreimageVerification.ts` — Preimágenes de Hashes HTLC**
  - [x] Detección de OP_SHA256, OP_HASH160, OP_HASH256, OP_RIPEMD160.
  - [x] Verificación `hash(preimage) == expected_hash` del script.
  - [x] Soporte completo para submarine swaps Boltz (HASH160).
  - [x] `verifyDAGHashPreimages()` — traversal recursivo.

- [x] **`vtxoDAGVerification.ts` — Reconstrucción y Verificación del DAG**
  - [x] Pipeline `reconstructAndValidateVtxoDAG()` — 6 pasos validados.
  - [x] Modo directo y modo privacidad (batch fetch).
  - [x] Detección de ciclos, huérfanos y breaks de chaining.
  - [x] Validación de conservación de sats (amount mismatch detection).
  - [x] Validación de Checkpoint transactions (sweep delay coherence).
  - [x] `verifyVtxoComplete()` + `verifyOnchainAnchoring()` para confirmaciones.

- [x] **`arkdProvider.ts` — Comunicación con Proveedor ARK**
  - [x] `getBatchVtxos()` — GET `/v1/batch/{txid}/vtxos`.
  - [x] `getVirtualTxs()` — POST `/v1/virtual-txs` con batching (50 txids/req).
  - [ ] *(Pendiente)* `getVtxoChain()` cuando arkd exponga el endpoint directo.

- [x] **`bitcoinRpc.ts` — Verificación contra Bitcoin RPC**
  - [x] `getRawTransaction()`, `getTxStatus()`, `getBlockchainInfo()`.
  - [x] `broadcastTransaction()` para sovereign exit.
  - [x] Detección de Oracle Poisoning (validación de formato TXID).
  - [x] `verifyCommitmentDepth()` para finality checks.

- [x] **`authenticator.ts` & `sovereignStorage.ts` — Auth y Storage Local**
  - [x] PBKDF2 (100k iteraciones, AES-256-GCM) para master key.
  - [x] Persist/retrieve del exit sequence cifrado en storage.
  - [x] `onReceiveVtxo()` — webhook automatizado al recibir un VTXO.
  - [x] `executeSovereignExit()` — salida unilateral sin conexión al ASP.

- [x] **`performanceUtils.ts` & `__tests__/` — Rendimiento y Pruebas**
  - [x] `ConcurrencyLimiter` — máx 10 RPC concurrentes.
  - [x] `VerificationCache` — TTL 5 min para evitar re-verificación.
  - [x] 104 tests en 8 archivos: unit, security, stress (depth=500), DoS, fuzzing, E2E.
  - [ ] *(Pendiente)* `arkd_integration.test.ts` requiere arkd en `localhost:18080`.
