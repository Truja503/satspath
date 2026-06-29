# SatsPath Swap Engine & ARK Bridge Integration

Hemos completado exitosamente la implementación del motor de interoperabilidad **`satspath-swaps`** y la integración nativa del **ARK SDK** mediante un Bridge JSON-RPC.

## 1. El Swap Engine (`satspath-swaps`)

Se creó un nuevo crate en el workspace encargado exclusivamente de manejar los intercambios entre capas mediante la API de Boltz v2.

- **BoltzClient:** Un cliente HTTP/REST asíncrono y WebSocket que negocia los fees, los límites y crea las transacciones de intercambio (`Submarine`, `Reverse` y `Chain`).
- **Persistencia Segura (AES-256-GCM):** Todos los secretos criptográficos generados en el lado del cliente (por ejemplo, los *preimages* de hash y llaves temporales de reclamo/reembolso) se cifran mediante `AES256-GCM` y se guardan en `.satspath/swaps.enc` o `swaps.json` según el entorno, garantizando la custodia segura antes de que las transacciones se asienten on-chain.
- **Flujos Implementados:**
  - `submarine.rs`: De Bitcoin On-chain / Ark VTXO hacia Lightning Network (el usuario envía a un lockup, Boltz paga el invoice).
  - `reverse.rs`: De Lightning Network hacia Bitcoin On-chain (Boltz retiene los fondos hasta que se revela el preimage).
  - `chain_swap.rs`: Entre capas On-chain o Ark, utilizando firmas Schnorr y Taproot Key-path para una máxima privacidad cooperativa y permitiendo offboarding de VTXOs.

## 2. ARK JSON-RPC Bridge

Para reutilizar la lógica ya probada en TypeScript para la validación de los VTXO DAGs de Arkade y la preparación de las salidas soberanas, implementamos un Bridge:

- **TypeScript Daemon (`ark-bridge`):** Un proceso en Node.js que importa directamente el SDK de validación nativo (incluyendo `reconstructAndValidateVtxoDAG` y `onReceiveVtxo`).
- **Rust Client (`ark_bridge.rs`):** El SwapManager invoca este bridge comunicándose a través de `stdin`/`stdout` usando el estándar `JSON-RPC`.
- **Integración:** Antes de aceptar un VTXO o iniciar un swap desde Arkade, el Rust CLI puede solicitar de forma síncrona la validación completa del DAG y asegurar que los datos soberanos estén almacenados en el disco local a través del bridge.

## 3. Router y CLI

- **Actualización del Router:** La selección de rutas en `router.rs` ahora expone un `SwapDirective` detallado (`SubmarineSwap`, `ChainSwap`, `ArkTransfer`, etc.) y calcula dinámicamente un **Dust Threshold** on-chain, rebotando automáticamente transacciones antieconómicas basándose en los `sat/vB` actuales de la mempool.
- **CLI (`pay.rs`):** Se abandonó el modelo 100% simulado. Ahora el CLI inicializa el `BoltzClient` y el `SwapManager`, levanta el `ArkBridge` (si está compilado), y manda peticiones reales de intercambio de red a Boltz Testnet. 

> [!NOTE]
> Las transacciones en el CLI en este MVP están configuradas apuntando a **Boltz Testnet** por defecto y el CLI detiene el proceso solicitando al usuario depositar manualmente fondos on-chain de prueba (Testnet BTC). 
> La construcción completa local de transacciones Taproot con firmas combinadas MuSig2 queda documentada (Phase 4b) en el código fuente para una iteración posterior si se agrega un monedero nativo en Rust.
