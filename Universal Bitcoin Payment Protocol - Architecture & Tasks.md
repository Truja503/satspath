# Universal Bitcoin Payment Protocol - Arquitectura y Tareas

A continuación, se resuelven las preguntas clave sobre la arquitectura de identidad y resolución, junto con la tabla de mitigación de riesgos y la lista de tareas (To-Do List) para la validación del lado del cliente de ARK.

## Preguntas de Arquitectura

**¿Quién firma el profile?**
*   **MVP:** El servidor o un servicio centralizado que actúa como registro tras validar el control del correo electrónico.
*   **Futuro:** El usuario firma su propio perfil utilizando una clave privada (Self-Sovereign Identity), y el servidor solo distribuye el perfil firmado (similar a Nostr).

**¿Qué pasa si cambia la llave?**
*   **MVP:** El perfil anterior se invalida o se sobrescribe, requiriendo una nueva validación del correo electrónico o cuenta.
*   **Futuro:** Se implementa un historial de rotación de llaves (Key rotation). Si una llave cambia repentinamente, se muestra una advertencia al remitente (modelo Trust On First Use - TOFU), mostrando el fingerprint de la llave.

**¿Cómo detectamos perfil falso?**
*   **MVP:** Mediante un *email challenge* (envío de código o enlace mágico) para asegurar que quien registra el perfil tiene acceso al correo, marcando perfiles sin validar como "no verificados".
*   **Futuro:** Integración de *Web of Trust* (Nostr), validación criptográfica atada a DNS (como NIP-05) o pruebas de identidad descentralizadas (Decentralized Identifiers - DIDs).

**¿Qué pasa si alguien registra rodrigo@gmail.com sin ser dueño?**
*   **MVP:** Al intentar registrar, el sistema enviará un correo a `rodrigo@gmail.com`. Sin acceso al correo, el atacante no podrá confirmar el registro. El perfil se mantiene en estado pendiente o no verificado y no es resuelto por la aplicación cliente.
*   **Futuro:** Verificación criptográfica mediante firmas del dominio (DNS) o identidad federada (OIDC) para prevenir totalmente el secuestro de alias.

**¿Qué información se guarda localmente?**
*   **En el cliente:** Caché de perfiles previamente resueltos y verificados, *fingerprints* de las llaves públicas de los contactos (para detectar ataques de reemplazo), preferencias de enrutamiento (prioridad de red, límites de fees) y el historial de pagos.

**¿Qué NO podemos prometer?**
*   No podemos prometer resistencia total a la censura si dependemos de un servidor central de resolución en el MVP (el servidor podría negarse a resolver un correo).
*   No podemos prometer privacidad absoluta de los metadatos de quién paga a quién si la consulta no es ofuscada.
*   No podemos evitar los *fees* subyacentes de la red seleccionada (on-chain o Lightning).

---

## Tabla de Amenazas y Mitigación

| Threat | Risk | MVP mitigation | Future mitigation |
| :--- | :--- | :--- | :--- |
| Fake email registration | High | mark as unverified | email challenge / DNS / Nostr verification |
| Key replacement attack | Medium | show key fingerprint | key history + warnings |
| Server tampering | High | signed profiles | decentralized registry |

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
