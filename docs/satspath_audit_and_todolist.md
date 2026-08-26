# 🔍 Auditoría SatsPath — Qué Está Implementado, Qué Falta, y Plan de Ramas

**Fecha:** Julio 2026  
**Rama actual:** `feat/satspath-receiver-flow` (5 commits adelante de `main`)  
**Repo:** `Truja503/satspath`  
**Wallet:** `/home/chelo/antigravity/PlanB/satspath/wallet` (Arkade Money)

---

## 1. Estado de Ramas (El "Desvergue")

### Diagnóstico

El repo tiene **18 ramas remotas** y **3 locales**. La `main` tiene todo el código Rust compilado con 277 tests. La rama actual (`feat/satspath-receiver-flow`) trajo la wallet completa al monorepo. Hay un `master` viejo que es un ancestro totalmente distinto.

### Ramas YA MERGEADAS en `main` (seguras de eliminar)

Estas ramas ya están contenidas en `main` — su trabajo ya está ahí:

| Rama                                             | Contenido              | Acción                     |
| ------------------------------------------------ | ---------------------- | -------------------------- |
| `origin/chore/remove-stray-scratch-files`        | Limpieza de archivos   | ✅ Borrar                  |
| `origin/codex/p2p-daemon-resolver`               | Wire daemon P2P        | ✅ Borrar                  |
| `origin/codex/p2p-publish-repair`                | Reparar P2P publish    | ✅ Borrar                  |
| `origin/feat/bip353-dns-resolution`              | BIP-353 DNS            | ✅ Borrar                  |
| `origin/feat/holepunch-p2p-sdk`                  | Holepunch P2P SDK      | ✅ Borrar                  |
| `origin/feat/peer-export-import`                 | Export/import perfiles | ✅ Borrar                  |
| `origin/feat/quote-json-contract`                | QuoteResponse JSON     | ✅ Borrar                  |
| `origin/feat/swap-engine-ark-bridge`             | Swap engine Ark        | ✅ Borrar                  |
| `origin/feat/wallet-profile-manager`             | Wallet profile mgr     | ✅ Borrar                  |
| `origin/feature/ark-send-receive-swaps`          | Ark send/receive       | ✅ Borrar                  |
| `origin/feature/payment-method-ownership-proofs` | Ownership proofs       | ✅ Borrar                  |
| `origin/feature/protocol-invite-routing-model`   | Invites + routing      | ✅ Borrar                  |
| `origin/fix/v0-priority-issues`                  | Fixes de prioridad     | ✅ Borrar                  |
| `origin/feature/mainnet-preview-mode-v2`         | Mainnet preview        | ✅ Borrar                  |
| `origin/feature/mainnet-ln-test`                 | LN test mainnet        | ✅ Borrar (ancestro viejo) |

### Ramas NO mergeadas (tienen trabajo único)

| Rama                                            | Commits extra | Contenido                          | Acción                                                           |
| ----------------------------------------------- | ------------- | ---------------------------------- | ---------------------------------------------------------------- |
| `origin/feat/satspath-receiver-flow`            | **5**         | Wallet + docs arquitectura         | **MERGEAR a `main`**                                             |
| `origin/feat/ark-vtxo-verification-integration` | **5**         | Verificación VTXO Ark              | ⚠️ Revisar: basada en `master` viejo, probablemente incompatible |
| `origin/feat/arkade-wallet-integration`         | **4**         | Wallet integration vieja           | ⚠️ Supersedida por `feat/satspath-receiver-flow` → **Borrar**    |
| `origin/master`                                 | —             | Ancestro original (Initial commit) | **Borrar** — todo migró a `main`                                 |

### 🔧 Plan de Limpieza (Comandos)

```bash
# 1. Mergear la rama actual a main, abortando si falla
git checkout main
git merge feat/satspath-receiver-flow --no-ff -m "feat: merge wallet + receiver flow into main" || {
  echo "Merge falló. Resuelva los conflictos antes de borrar cualquier rama." >&2
  exit 1
}

# 2. Respaldar las referencias de todas las ramas remotas antes de borrar
git ls-remote --heads origin > branch-tips-backup-$(date +%F).txt

# 3. Borrar de forma segura solo las ramas remotas contenidas en main
# Cambie DRY_RUN=0 para ejecutar la eliminación real
DRY_RUN=1

for b in \
  chore/remove-stray-scratch-files \
  codex/p2p-daemon-resolver \
  codex/p2p-publish-repair \
  feat/bip353-dns-resolution \
  feat/holepunch-p2p-sdk \
  feat/peer-export-import \
  feat/quote-json-contract \
  feat/swap-engine-ark-bridge \
  feat/wallet-profile-manager \
  feature/ark-send-receive-swaps \
  feature/payment-method-ownership-proofs \
  feature/protocol-invite-routing-model \
  fix/v0-priority-issues \
  feature/mainnet-preview-mode-v2 \
  feature/mainnet-ln-test \
  feat/arkade-wallet-integration; do
  if git merge-base --is-ancestor "origin/$b" main 2>/dev/null; then
    if [ "$DRY_RUN" = "1" ]; then
      echo "[DRY-RUN] SE BORRARÍA (contenida en main): $b"
    else
      echo "Borrando rama remota: $b"
      git push origin --delete "$b"
    fi
  else
    echo "[OMITIDA] $b NO está contenida completamente en main"
  fi
done

# 4. Manejo manual de master y ramas históricas:
#    - 'feat/ark-vtxo-verification-integration' debe revisarse primero para cherry-picks.
#    - 'master' solo debe borrarse tras confirmar que ningún commit pendiente depende de ella.

# 5. Limpiar ramas locales fusionadas
git branch -d feat/satspath-receiver-flow fix/v0-priority-issues feat/swap-engine-ark-bridge 2>/dev/null || true

# 6. Limpiar referencias locales obsoletas
git remote prune origin
```

> **IMPORTANT:**
> Después de esto, el repo quedará con solo `main` y opcionalmente `feat/ark-vtxo-verification-integration` si decides cherry-pick.

---

## 2. Qué SÍ está implementado (verificado en código)

### ✅ Core del Protocolo — [satspath-core](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src)

| Feature                                          | Archivo(s)                                                                                                                                                                                                   | Estado                       |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------- |
| `PaymentProfile` con Lightning, Onchain, Ark     | [profile.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/profile.rs)                                                                                                              | ✅ Completo                  |
| `SignedPaymentProfile` con firma Schnorr         | [profile.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/profile.rs#L177-L182)                                                                                                    | ✅ Completo                  |
| Generación de keypair secp256k1                  | [crypto.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/crypto.rs#L20-L27)                                                                                                        | ✅ Completo                  |
| Firma con domain separator (`SatsPathProfileV1`) | [crypto.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/crypto.rs#L43-L60)                                                                                                        | ✅ Completo                  |
| Verificación de firma Schnorr                    | [crypto.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/crypto.rs#L65-L87)                                                                                                        | ✅ Completo                  |
| Verificación de expiración                       | [crypto.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/crypto.rs#L97-L108)                                                                                                       | ✅ Completo                  |
| Nonces de replay protection                      | [crypto.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/crypto.rs#L123-L128)                                                                                                      | ✅ Completo                  |
| Canonical JSON serialization                     | [crypto.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/crypto.rs#L31-L37)                                                                                                        | ✅ Completo                  |
| Validación de perfiles públicos                  | [validation.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/validation.rs#L233-L334)                                                                                              | ✅ Completo                  |
| Rechazo de material privado (xprv, seeds, etc.)  | [validation.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/validation.rs#L112-L144)                                                                                              | ✅ Completo                  |
| Validación de Bitcoin Address + red              | [validation.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/validation.rs#L98-L110)                                                                                               | ✅ Completo                  |
| Validación de Lightning Address                  | [validation.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/validation.rs#L39-L63)                                                                                                | ✅ Completo                  |
| Validación de BOLT12 offer (bech32 `lno`)        | [validation.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/validation.rs#L68-L96)                                                                                                | ✅ Completo                  |
| Invitaciones firmadas                            | [lib.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/lib.rs#L86-L127)                                                                                                             | ✅ Completo                  |
| Verificación de invitaciones                     | [lib.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/lib.rs#L130-L153)                                                                                                            | ✅ Completo                  |
| Key Rotation (tipos + validación)                | [rotation.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/rotation.rs)                                                                                                            | ✅ Tipos + lógica            |
| Ownership Proofs (firmas + well-known)           | [ownership.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/ownership.rs)                                                                                                          | ✅ Extenso (73KB)            |
| Privacy (identifier hash, masking)               | [privacy.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/privacy.rs)                                                                                                              | ✅ Completo                  |
| BIP-321 parsing (bitcoin: URI)                   | [bip321.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/bip321.rs)                                                                                                                | ✅ Completo                  |
| BIP-353 resolve + publish                        | [bip353.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/bip353.rs), [bip353_publish.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/bip353_publish.rs) | ✅ Completo (20KB + 16KB)    |
| Ark: receive pointer, validation, ownership      | [ark.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/ark.rs)                                                                                                                      | ✅ Tipos + validación (18KB) |
| Peer Registry (local P2P)                        | [peer_registry.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/peer_registry.rs)                                                                                                  | ✅ Completo                  |
| Local registry                                   | [registry.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/registry.rs)                                                                                                            | ✅ Completo                  |
| Split Payment (tipos)                            | [split.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/split.rs)                                                                                                                  | ⚠️ Solo tipos (1.8KB)        |
| Payment Pointer                                  | [pointer.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/pointer.rs)                                                                                                              | ✅ Completo                  |
| Codec (universal request encoding)               | [codec.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/codec.rs)                                                                                                                  | ✅ Completo                  |

### ✅ Resolver Chain — [satspath-core/resolvers](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/resolvers)

| Resolver                                   | Archivo                                                                                                     | Estado                                                               |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `ChainResolver` (compositor)               | [resolver.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/resolver.rs)           | ✅ Con protección anti-sustitución (SEC-02)                          |
| `HttpResolver` (HTTPS well-known + NIP-05) | [http.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/resolvers/http.rs)         | ✅ 10KB, verifica firma + expiración                                 |
| `Bip353Resolver` (DNS TXT)                 | [bip353.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/resolvers/bip353.rs)     | ✅ Funcional                                                         |
| `NostrResolver` (NIP-05)                   | [nostr.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/resolvers/nostr.rs)       | ✅ 14KB                                                              |
| `PearResolver` (Hyperswarm P2P)            | [pear.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/resolvers/pear.rs)         | ⚠️ Funcional pero usa `Command::new("node")` — depende de repo paths |
| `PlatformResolver` (placeholder)           | [platform.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/resolvers/platform.rs) | ❌ Solo 494 bytes — stub                                             |

### ✅ Router — [satspath-router](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src)

| Feature                                      | Archivo                                                                                                           | Estado                                                 |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| `select_route()` (Lightning → Onchain → Ark) | [router.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/router.rs)                   | ✅ Completo + tests                                    |
| Fee estimation (mempool.space API)           | [fees.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/fees.rs)                       | ✅ Funcional                                           |
| Scoring engine                               | [scoring.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/scoring.rs)                 | ✅ 14KB                                                |
| Priority routing                             | [priority.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/priority.rs)               | ✅ 7.8KB                                               |
| Urgency levels                               | [urgency.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/urgency.rs)                 | ✅ Completo                                            |
| LNURL-pay 2-step (metadata → invoice)        | [lightning.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/lightning.rs)             | ✅ Completo con validación BOLT11                      |
| BOLT12 handling                              | [bolt12.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/bolt12.rs)                   | ⚠️ Tipos + parsing (10KB), sin flujo completo          |
| Silent Payments                              | [silent_payments.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/silent_payments.rs) | ⚠️ Tipos + validación (10KB), sin test con wallet real |
| Split Payments                               | [split_payments.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/split_payments.rs)   | ⚠️ Validación (6.6KB), sin atomicidad ni ejecución     |
| Key Rotation router                          | [key_rotation.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/key_rotation.rs)       | ⚠️ Tipos + validación                                  |
| Ark routes                                   | [ark_routes.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/ark_routes.rs)           | ⚠️ Planificación (7.7KB), sin test con ASP real        |
| QuoteResponse contract                       | [quote_response.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/quote_response.rs)   | ✅ 23KB contrato estable                               |
| BIP-353 preview                              | [bip353_preview.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/bip353_preview.rs)   | ✅ Funcional                                           |

### ✅ WASM — [satspath-wasm](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-wasm/src)

| Feature                       | Archivo                                                                                                | Estado                 |
| ----------------------------- | ------------------------------------------------------------------------------------------------------ | ---------------------- |
| `generate_identity_keypair()` | [crypto.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-wasm/src/crypto.rs#L34-L42)  | ✅ Funcional           |
| `sign_profile_json()`         | [crypto.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-wasm/src/crypto.rs#L47-L75)  | ✅ Funcional           |
| `verify_signed_profile()`     | [crypto.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-wasm/src/crypto.rs#L94-L164) | ✅ Con legacy fallback |
| `quote()` (resolve + route)   | [router.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-wasm/src/router.rs)          | ✅ 17KB                |
| Resolver chain WASM           | [resolver.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-wasm/src/resolver.rs)      | ✅ 15KB                |
| `topic_for_alias()`           | [topic.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-wasm/src/topic.rs)            | ✅ Funcional           |
| Types + constants             | [types.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-wasm/src/types.rs)            | ✅ 9KB                 |

### ✅ FFI (UniFFI → Kotlin/Swift) — [satspath-ffi](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-ffi/src)

| Feature                      | Archivo                                                                                          | Estado                               |
| ---------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------ |
| Identity management          | [identity.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-ffi/src/identity.rs) | ✅ Funcional                         |
| Profile creation + signing   | [profile.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-ffi/src/profile.rs)   | ✅ Funcional                         |
| Crypto (verify, fingerprint) | [crypto.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-ffi/src/crypto.rs)     | ✅ Funcional                         |
| Resolver FFI                 | [resolver.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-ffi/src/resolver.rs) | ✅ Funcional                         |
| Router FFI                   | [router.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-ffi/src/router.rs)     | ✅ Funcional                         |
| Type conversions             | [convert.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-ffi/src/convert.rs)   | ✅ 12KB                              |
| **P2P bridge**               | [p2p.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-ffi/src/p2p.rs)           | ❌ **Placeholder** — `TODO(Phase 5)` |

### ✅ Daemon (satspathd) — [satspathd](file:///home/chelo/antigravity/PlanB/satspath/crates/satspathd/src)

| Feature                                                             | Estado                         |
| ------------------------------------------------------------------- | ------------------------------ |
| HTTP API (health, profile, resolve, quote, pay, send, receive, dns) | ✅ Funcional (58KB main.rs)    |
| UI HTML embebida                                                    | ✅ Funcional (22KB index.html) |
| QR SVG generation                                                   | ✅ Funcional                   |

### ✅ Wallet Integration (Arkade Money) — [wallet](file:///home/chelo/antigravity/PlanB/satspath/wallet)

| Feature                                   | Archivo                                                                                                              | Estado                                             |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| WASM bridge (`satspath.ts`)               | [satspath.ts](file:///home/chelo/antigravity/PlanB/satspath/wallet/src/lib/satspath.ts)                              | ✅ `routePayment()` + `buildSatsPathProfile()`     |
| Receiver UI (Settings → SatsPath Profile) | [SatsPathProfile.tsx](file:///home/chelo/antigravity/PlanB/satspath/wallet/src/screens/Settings/SatsPathProfile.tsx) | ✅ Genera keypair + firma + guarda en LocalStorage |
| Send Form interceptor                     | [Form.tsx](file:///home/chelo/antigravity/PlanB/satspath/wallet/src/screens/Wallet/Send/Form.tsx)                    | ✅ Detecta alias `@` → rutea con SatsPath          |
| Storage (LocalStorage)                    | [storage.ts](file:///home/chelo/antigravity/PlanB/satspath/wallet/src/lib/storage.ts)                                | ✅ Save/read profile                               |
| LaserBeam animation                       | LaserBeam.tsx                                                                                                        | ✅ Efecto visual durante generación                |

---

## 3. Qué NO está implementado (verificado en código)

### ❌ Crítico para producción

| Item                                     | Detalle                                                                                                                                                                          | Dificultad                |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------- |
| **Verificación real de identidad**       | `MockEmailVerifier` en [platform.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/platform.rs#L37-L62). Magic links, tokens, rate limiting — nada real | 🔴 Alta                   |
| **Procedencia del perfil (trust model)** | `ResolvedProfile` con `source`, `trust_level`, `identifier_verified` no existe. El resolver devuelve solo `SignedPaymentProfile`                                                 | 🔴 Alta                   |
| **Publicación real del perfil**          | La wallet genera el JSON firmado pero NO lo publica. Solo "Copy to clipboard". No hay publish a Nostr, DNS, ni HTTPS                                                             | 🔴 Alta                   |
| **Seguridad del daemon**                 | Sin auth, sin CSRF, CORS abierto, sin rate limiting, sin separación admin/public                                                                                                 | 🔴 Alta                   |
| **Protección de clave de identidad**     | Secret key guardada en LocalStorage en texto plano (wallet) y archivos JSON (daemon). Sin Keychain/Keystore                                                                      | 🔴 Alta                   |
| **SSRF protection**                      | Los resolvers HTTP/LNURL no bloquean localhost/redes privadas                                                                                                                    | 🟡 Media                  |
| **P2P embebido (FFI)**                   | [p2p.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-ffi/src/p2p.rs) es un placeholder con `TODO(Phase 5)`                                                     | 🟡 Media                  |
| **WASM .wasm binary**                    | El wallet importa `satspath-wasm` pero NO hay el archivo `satspath_wasm_bg.wasm` compilado en `public/`                                                                          | 🔴 Crítico para funcionar |

### ⚠️ Parcialmente implementado

| Item                    | Estado actual                                                                                                                                                                                                                                                   | Qué falta                                                                             |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| **BOLT12**              | Validación de formato bech32 (`lno`) en [validation.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/validation.rs#L68-L96), tipos en [bolt12.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-router/src/bolt12.rs) | Flujo completo: offer → invoice request → invoice → verificación → handoff            |
| **Silent Payments**     | Tipos + validación de SP pubkey, campo `silent_payment_pubkey` en Onchain                                                                                                                                                                                       | Generación BIP-352, derivación ephemeral address, test con wallet real                |
| **Split Payments**      | Solo structs en [split.rs](file:///home/chelo/antigravity/PlanB/satspath/crates/satspath-core/src/split.rs) (1.8KB), validación en router                                                                                                                       | Ejecución multi-rail, atomicidad, manejo de fallo parcial                             |
| **Ark interop**         | Tipos + validación de pointer/proof, routing a Ark como fallback, `ArkadeManual` SwapDirective                                                                                                                                                                  | Test con ASP real (Arkade), formato de receive pointer compatible, flujo Ark-to-Ark   |
| **Key Rotation**        | Tipos + `rotate_identity_key()` + validación                                                                                                                                                                                                                    | Integración con resolvers (rechazar replays), propagación a publicaciones             |
| **BIP-353 DNSSEC**      | Resolución DNS funcional con DoH                                                                                                                                                                                                                                | DNSSEC operativo, publicación automatizada, rotación TTL                              |
| **PearResolver**        | Funcional pero spawndea `node index.js`                                                                                                                                                                                                                         | Depende de paths relativos del repo — no funciona como app instalada                  |
| **Send Flow en Wallet** | Intercepta `@` aliases, rutea, muestra método + fee                                                                                                                                                                                                             | No ejecuta el pago vía WASM — solo muestra info. Mock hardcoded para `chello@dev.idk` |

### ❌ No implementado en absoluto

| Item                                       | Detalle                                                                                                                                             |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| P2P soberano (ECDH, ChaCha20, SYN/SYN-ACK) | Plan en [satspath_architecture_wallet.md](file:///home/chelo/antigravity/PlanB/satspath/docs/satspath_architecture_wallet.md#L155-L169) — 0% código |
| Web of Trust (NIP-65)                      | Solo en docs — 0% código                                                                                                                            |
| Hashcash anti-spam                         | Solo en docs — 0% código                                                                                                                            |
| Métricas / Observabilidad                  | Nada implementado                                                                                                                                   |
| CI obligatorio (gate)                      | Workflow existe pero no es gate                                                                                                                     |
| Releases firmadas                          | No hay releases                                                                                                                                     |
| Test vectors públicos                      | No existen                                                                                                                                          |
| Auditoría de criptografía                  | No realizada                                                                                                                                        |

---

## 4. TODO List — SatsPath → Producción + Integración Wallet

### Fase 0: Limpieza (1-2 días)

- [ ] Mergear `feat/satspath-receiver-flow` → `main`
- [ ] Borrar las 15+ ramas mergeadas/obsoletas (ver comandos arriba)
- [ ] Verificar que `cargo test` pasa en main con la wallet incluida
- [ ] Compilar `satspath-wasm` y colocar `.wasm` en `wallet/public/`
- [ ] Eliminar el mock hardcoded `chello@dev.idk` de [satspath.ts](file:///home/chelo/antigravity/PlanB/satspath/wallet/src/lib/satspath.ts#L49-L55)

---

### Fase 1: MVP Funcional (1-2 semanas)

#### 1.1 WASM Build Pipeline

- [ ] Script/Makefile: `wasm-pack build crates/satspath-wasm --target web --out-dir ../../wallet/public/`
- [ ] Verificar que la wallet carga el WASM y puede generar keypairs
- [ ] Verificar que `quote()` funciona end-to-end en browser

#### 1.2 Publicación del Perfil

- [ ] Implementar publish a Nostr (NIP-01 event con perfil firmado) desde el WASM bridge
- [ ] Alternativa: POST al daemon `satspathd` que publique vía Nostr relay
- [ ] Agregar botón "Broadcast to Nostr" en [SatsPathProfile.tsx](file:///home/chelo/antigravity/PlanB/satspath/wallet/src/screens/Settings/SatsPathProfile.tsx)

#### 1.3 Resolución Real en Send

- [ ] En [Form.tsx](file:///home/chelo/antigravity/PlanB/satspath/wallet/src/screens/Wallet/Send/Form.tsx): cuando SatsPath devuelve `RouteResult`:
  - Si es Lightning → usar el payload como Lightning Address y continuar con el flujo existente de LN de la wallet
  - Si es Onchain → construir BIP-21 URI y continuar con flujo onchain existente
  - Si es Ark → manejar con wallet adapter de Arkade

#### 1.4 Trust Model Básico

- [ ] Agregar `ResolvedProfile` wrapper con `source: ResolverSource` al resultado del WASM `quote()`
- [ ] Mostrar indicador visual en la wallet: "Verificado por DNS" / "Verificado por Nostr" / "Sin verificar"

---

### Fase 2: Seguridad Avanzada y Resistencia Cuántica (1-2 semanas)

#### 2.1 Protección de Secret Key

- [ ] En la wallet: cifrar `secretKey` en LocalStorage con password del usuario o derivar de Arkade wallet seed
- [ ] En satspathd: migrar de JSON plano a Keychain/Secret Service (ya existe code path AES-GCM, verificar que funcione)

#### 2.2 Verificación de Identidad

- [ ] Implementar `EmailVerifier` real con:
  - [ ] Token aleatorio de un solo uso
  - [ ] Expiración (10 min)
  - [ ] Rate limiting
- [ ] Para dominios: verificar ownership via DNS TXT record o HTTPS well-known

#### 2.3 Daemon Hardening

- [ ] Auth token para API admin
- [ ] CORS restrictivo
- [ ] SSRF: bloquear resolvers a localhost/private nets para contenido remoto
- [ ] Rate limiting
- [ ] Max payload size

#### 2.4 Criptografía Post-Cuántica (PQC) y Semi-Cuántica

- [ ] **Esquemas Híbridos de Firma:** Añadir soporte para firmas híbridas combinando ECDSA/Schnorr tradicional con **ML-DSA (Dilithium)**. Esto asegura compatibilidad actual mientras protege las identidades contra futuros ataques de computación cuántica (Shor's algorithm).
- [ ] **Intercambio de Claves P2P:** Transicionar el P2P ECDH (Fase 5) a un KEM híbrido que combine curvas elípticas (X25519) con **ML-KEM (Kyber)** para establecer los secretos compartidos.
- [ ] **Migración de Perfiles:** Definir un field en el JSON del profile para anunciar soporte PQC y transicionar perfiles clásicos a PQC de forma fluida.

---

### Fase 3: Integración Completa con Arkade Wallet (2-3 semanas)

#### 3.1 Receiver Flow Completo

- [ ] Crear perfil → Firmar → Publicar a Nostr/DNS → Confirmar publicación
- [ ] Mostrar QR del perfil firmado
- [ ] Permitir actualización del perfil (increment sequence)
- [ ] Manejar expiración y renovación automática

#### 3.2 Sender Flow Completo

- [ ] Resolve → Verify → Quote → Confirm → Execute Payment
- [ ] Lightning: obtener invoice BOLT11 vía LNURL → pagar con wallet LN
- [ ] Onchain: construir tx con BIP-21 → firmar con wallet onchain
- [ ] Ark: construir intent → enviar vía Arkade SDK

#### 3.3 Wallet Adapter

- [ ] Interfaz `WalletAdapter` en el WASM bridge que reporte:
  - [ ] Balance por rail (LN/onchain/Ark)
  - [ ] Capacidad de pago
  - [ ] Resultado del pago (éxito/fallo)
- [ ] Integrar con providers existentes de Arkade wallet (`WalletContext`, `svcWallet`)

#### 3.4 Fallback entre Rails

- [ ] Si Lightning falla → intentar Ark → intentar Onchain
- [ ] Mostrar al usuario el fallback con razón legible

---

### Fase 4: Rails Avanzados (3-4 semanas)

- [ ] **Ark interop real**: probar con ASP de Arkade (formato de pointer, URI, QR)
- [ ] **BOLT12**: implementar flujo completo offer → invoice con wallet LDK
- [ ] **Silent Payments**: generación BIP-352 + derivación ephemeral address
- [ ] **Split Payments**: semántica definida (multi-receiver vs multi-rail) + ejecución

---

### Fase 5: P2P Soberano (4-6 semanas)

- [ ] **Derivación de tópico seguro** (no alias en texto plano)
- [ ] **ECDH compartido** para P2P cifrado
- [ ] **Protocolo SYN/SYN-ACK** para pagos P2P
- [ ] **Web of Trust** con NIP-65
- [ ] **Hashcash anti-spam**
- [ ] Migrar PearResolver de `Command::new("node")` a transport embebido

---

### Fase 6: Producción (2-3 semanas)

- [ ] CI como gate obligatorio
- [ ] Releases versionadas + changelog
- [ ] Test vectors públicos
- [ ] Auditoría de criptografía externa
- [ ] Fuzzing de perfiles y URIs
- [ ] Documentación API estable

---

## 5. Resumen Rápido

```
╔════════════════════════════════════════╗
║        SATSPATH STATUS                 ║
╠════════════════════════════════════════╣
║ Core (crypto + profiles + validation)  ║
║ ██████████████████████████████ 95%     ║
║                                        ║
║ Resolver Chain                         ║
║ ████████████████████████░░░░░░ 80%     ║
║                                        ║
║ Router + Scoring                       ║
║ ████████████████████████░░░░░░ 80%     ║
║                                        ║
║ WASM Bindings                          ║
║ ██████████████████████████░░░░ 85%     ║
║                                        ║
║ FFI (Kotlin/Swift)                     ║
║ ██████████████████░░░░░░░░░░░░ 60%     ║
║                                        ║
║ Wallet Integration                     ║
║ ████████████░░░░░░░░░░░░░░░░░░ 40%     ║
║                                        ║
║ Publish + Resolve E2E                  ║
║ ██████░░░░░░░░░░░░░░░░░░░░░░░░ 20%    ║
║                                        ║
║ Security Hardening                     ║
║ ████░░░░░░░░░░░░░░░░░░░░░░░░░░ 15%    ║
║                                        ║
║ P2P Soberano                           ║
║ ██░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 5%     ║
╚════════════════════════════════════════╝
```

> **TIP:**
> **Prioridad inmediata**: Limpiar ramas → compilar WASM → que la wallet pueda hacer el flujo `Receiver: crear perfil + publicar` y `Sender: resolver + quote + mostrar confirmación` de verdad. Todo lo demás viene después.
