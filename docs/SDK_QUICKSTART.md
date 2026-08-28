# ⚡ SatsPath SDK Quickstart Guide

Guía rápida para desarrolladores de wallets, PWAs y aplicaciones web (React, Vite, React Native, Arkade).

---

## 📦 1. Instalación (con `pnpm`)

Instala los paquetes oficiales de SatsPath utilizando **`pnpm`**:

```bash
pnpm add @satspath/wasm bip39 react-qr-code
# o los paquetes de TypeScript modulares:
pnpm add @satspath/resolvers @satspath/router
```

> [!TIP]
> Si estás compilando el paquete WebAssembly directamente desde el repositorio local:
>
> ```bash
> cd crates/satspath-wasm
> wasm-pack build --target web --out-dir ../../pkg/satspath-wasm
> cd ../../your-app
> pnpm add ../satspath/pkg/satspath-wasm
> ```

---

## 🚀 2. Flujo Básico en 3 Pasos

```text
┌─────────────────────────┐     ┌─────────────────────────┐     ┌─────────────────────────┐
│  1. DERIVAR IDENTIDAD   │ ──> │   2. RESOLVER & QUOTE   │ ──> │    3. EJECUTAR PAGO     │
│  Seed (12 palabras)     │     │   "user@domain" + sats  │     │   QR / BOLT11 / BIP-21  │
│  m/9737'/0'             │     │   Smart Route Selection │     │   Wallet nativa paga    │
└─────────────────────────┘     └─────────────────────────┘     └─────────────────────────┘
```

---

## 💻 3. Código de Ejemplo (TypeScript / React)

### Paso 1: Inicializar el SDK y Derivar la Identidad

Deriva una clave de identidad segura y determinista dentro del entorno de la wallet sin tocar ni exponer las claves de gasto de Bitcoin:

```typescript
import init, { derive_identity_keypair_from_seed } from '@satspath/wasm';
import * as bip39 from 'bip39';

// 1. Inicializar el motor WASM al montar la aplicación
await init();

// 2. Derivar la identidad SatsPath dentro del entorno seguro de la wallet (m/9737'/0')
// La frase semilla permanece estrictamente dentro de la wallet y nunca se transmite.
const mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const seed = bip39.mnemonicToSeedSync(mnemonic);

// Cuenta 0 por defecto
const identity = derive_identity_keypair_from_seed(seed, 0);

console.log("Tu Clave Pública SatsPath:", identity.pubkey_hex);
// => "03e0fa79bc28965724d3eee52d58cf0cd11f712462582f42e79a545d13d85aac0b"
```

---

### Paso 2: Resolver un Alias y Cotizar la Mejor Ruta (`quote`)

Cuando el usuario ingresa un destinatario (ej. `chelo@satspath.dev`) y un monto en Satoshis:

```typescript
import { quote } from '@satspath/wasm';

async function handleSendPayment(recipientAlias: string, amountSats: bigint) {
  try {
    // Resuelve el alias y evalúa comisiones de mempool en vivo
    const quoteResult = await quote(recipientAlias, amountSats);

    if (quoteResult.status === "ok") {
      console.log("Riel Seleccionado:", quoteResult.selected_method.type); // "Lightning" | "Ark" | "Onchain"
      console.log("Comisión Estimada:", quoteResult.fee_sats, "sats");
      console.log("Payload de Pago:", quoteResult.qr); 
      
      // quoteResult.qr contiene el string listo para pagar:
      // - Si fue Lightning: "lnbc10u1p..." o BOLT12 offer
      // - Si fue Ark: "ark:<pubkey>?server=...&amount=10000"
      // - Si fue Onchain: "bitcoin:bc1q...?amount=0.0001"
      
      return quoteResult.qr;
    } else {
      console.error("No se pudo rutear el pago:", quoteResult.reason);
    }
  } catch (error) {
    console.error("Error resolviendo o cotizando pago:", error);
  }
}
```

---

### Paso 3: Renderizar el Código QR en la UI (React)

```tsx
import React from 'react';
import QRCode from 'react-qr-code';

export function PaymentScreen({ invoicePayload }: { invoicePayload: string }) {
  return (
    <div className="payment-card">
      <h3>Escanea para Pagar</h3>
      <div style={{ background: 'white', padding: '16px', borderRadius: '8px' }}>
        <QRCode value={invoicePayload} size={256} />
      </div>
      <p style={{ wordBreak: 'break-all', fontSize: '12px' }}>{invoicePayload}</p>
    </div>
  );
}
```

---

## 📖 4. Tabla de Métodos Principales

| Función | Parámetros | Retorno | Descripción |
| :--- | :--- | :--- | :--- |
| `derive_identity_keypair_from_seed` | `seed: Uint8Array, index: number` | `{ pubkey_hex, secret_key_hex }` | Deriva el par de claves de identidad determinísticamente bajo el namespace `m/9737'/0'`. |
| `quote` | `recipient: string, amount_sats: bigint` | `QuoteResponse` | Resuelve el alias a través de la cadena S2S/DNSSEC y selecciona el riel óptimo (Lightning, Ark u Onchain). |
| `resolve_alias` | `alias: string` | `SignedPaymentProfile` | Resuelve y obtiene el perfil firmado del destinatario (la firma se valida con `verify_signed_profile`). |
| `verify_signed_profile` | `profile_json: string` | `boolean` | Verifica la firma Schnorr `secp256k1` del perfil recibido. |

---

## 🛡️ 5. Principios de Seguridad para Wallets

1. **Zero Custody:** El SDK nunca almacena ni solicita claves privadas de Bitcoin (`xprv`/`tprv`).
2. **Fail-Closed:** Si un perfil tiene firma inválida o el DNSSEC falla, la función `quote` rechaza la transacción automáticamente.
3. **Mempool Smart Routing:** Evalúa Lightning primero, selecciona On-chain cuando la tarifa de red está por debajo del umbral, y recurre a Ark como alternativa off-chain.
