# Arquitectura de SatsPath en Arkade Wallet

El protocolo de **SatsPath** está completamente integrado en la wallet a través de un puente con WebAssembly (WASM). Esto permite que toda la criptografía pesada y la lógica de ruteo y resolución ocurran de forma ultra-rápida y segura del lado del cliente, utilizando el mismo código robusto en Rust de `satspath-core`.

A continuación te presento un diagrama de secuencia de cómo fluyen los datos en las dos etapas principales: el **Flujo del Receptor** (creación de identidad) y el **Flujo del Emisor** (resolución y envío).

```mermaid
sequenceDiagram
    participant User as Usuario
    participant Wallet as Arkade Wallet (React)
    participant Bridge as satspath.ts (WASM Bridge)
    participant WASM as satspath-wasm (Rust)
    participant Network as Red (Nostr / DNS)
    
    %% Flujo del Receptor
    rect rgb(35, 20, 50)
    Note over User, Network: 1. FLUJO DEL RECEPTOR (Crear Identidad)
    User->>Wallet: Escribe Alias y Direcciones (Settings)
    Wallet->>Wallet: Detona animación de Láseres 💥
    Wallet->>Bridge: buildSatsPathProfile()
    Bridge->>WASM: generate_identity_keypair()
    Note right of WASM: Genera llave secp256k1
    WASM-->>Bridge: IdentityKeypair (Hex)
    Bridge->>Bridge: Construye Perfil JSON crudo
    Bridge->>WASM: sign_profile_json(json, secretKey)
    Note right of WASM: Aplica hash y firma de Schnorr
    WASM-->>Bridge: Firma Criptográfica
    Bridge-->>Wallet: SignedPaymentProfile
    Wallet->>Wallet: Guarda en LocalStorage
    Wallet->>User: Muestra JSON firmado listo para la red
    end

    %% Flujo del Emisor
    rect rgb(50, 30, 20)
    Note over User, Network: 2. FLUJO DEL EMISOR (Enviar Sats)
    User->>Wallet: Enviar a "chelo@dev.idk"
    Wallet->>Bridge: routePayment()
    Bridge->>WASM: quote()
    WASM->>Network: Resuelve Perfil (BIP353/NIP05/Local)
    Network-->>WASM: SignedPaymentProfile
    Note right of WASM: Valida firmas y expiración
    WASM->>WASM: router::select_route()
    Note right of WASM: Sistema de Scoring elige <br/>el método más barato/óptimo
    WASM-->>Bridge: QuoteResponse (Mejor método)
    Bridge-->>Wallet: RouteResult (Ej. Lightning)
    Wallet->>User: Muestra pantalla de confirmación
    end
```

## Componentes Clave

1. **Arkade Wallet (React):** Es la capa de presentación. Maneja las animaciones (como los láseres), la recolección de datos del usuario, el guardado local y muestra los resultados finales.
2. **`satspath.ts` (WASM Bridge):** Es el traductor. Se encarga de convertir los objetos de JavaScript a los tipos primitivos que espera Rust, y viceversa. Aquí es donde inyectamos los "mocks" para desarrollo.
3. **`satspath-wasm` (Rust):** El cerebro de la operación. Hereda el código de `satspath-core`. Contiene el generador criptográfico seguro, la lógica matemática de las firmas de Schnorr, y el motor de Scoring que decide si es más barato irse por Ark, Lightning o On-chain dependiendo de las comisiones en tiempo real.

graph TD
    %% Flujo de Usuarios
    User((Usuario)) -- "Enviar Sats" --> Bob((Bob))
    Bob -- "Recibir Sats" --> QR["Muestra Código QR"]
    QR --> Alias["QR contiene Alias P2P"]
    Alias --> Address["bob@satspath.p2p"]
    Address --> PubKeys["Contiene Identity Pubkey"]

    %% Protocolos y Capas
    subgraph Capas de Red
        LN["Lightning Network"]
        ONCHAIN["On-chain"]
        ARK["Ark Protocol"]
        P2P["Red P2P (Hyperswarm)"]
    end

    %% Seguridad Soberana
    subgraph Security [Seguridad Soberana y Confianza P2P]
        Sec["Seguridad"]
        Enc["Cifrado P2P (ECDH)"]
        Id["Identidad Criptográfica (Pubkey secp256k1)"]
        
        %% Reemplazamos Email/Phone por métodos descentralizados
        QR_Scan["Verificación Out-of-band (QR Scan)"]
        WoT["Web of Trust (Gráfica Social Nostr)"]
        Hashcash["Prueba de Trabajo (Anti-Spam)"]
        
        Trust["Confianza Matemática y Social"]

        Sec --> Enc
        Sec --> Id
        Sec --> QR_Scan
        Sec --> WoT
        Sec --> Hashcash
        
        QR_Scan --> Trust
        WoT --> Trust
        Hashcash --> Trust
    end
