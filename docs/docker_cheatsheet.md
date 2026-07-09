# 🐳 SatsPath Docker Cheat Sheet

Esta guía contiene todos los comandos necesarios para construir, ejecutar y probar la funcionalidad completa de **SatsPath** usando los contenedores de Docker de manera segura.

> [!NOTE]
> Dado que el CLI se ejecuta de forma efímera, el formato base para correr cualquier comando es:
> `docker compose run --rm satspath-cli <comando>`

---

## 1. Construcción y Entorno Base

Antes de ejecutar comandos, necesitas construir las imágenes e iniciar el daemon en segundo plano.

```bash
# Construir todas las imágenes (CLI, Daemon y Ark Bridge)
docker compose build

# Levantar el Daemon local (satspathd) en segundo plano
docker compose up -d satspathd

# Verificar que el Daemon esté corriendo y saludable (Healthcheck)
docker compose ps
```

---

## 2. Inicialización de Billetera y Perfil

SatsPath es una billetera "receiver-profile". Gestiona tu identidad y tus métodos de pago públicos, sin custodiar fondos.

```bash
# 1. Inicializar las llaves de identidad criptográfica
docker compose run --rm satspath-cli wallet init

# 2. Registrar tu alias y configurar tus métodos de pago públicos
docker compose run --rm satspath-cli wallet add-methods bob@satspath.local \
    --lightning-address bob@getalby.com \
    --onchain-address tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx \
    --ark-server https://ark-testnet.example.com \
    --ark-pubkey 025b90f4a...

# 3. Ver tu perfil actual y el estado de la firma criptográfica
docker compose run --rm satspath-cli wallet show
```

---

## 3. Resolviendo Pagos (Simulaciones y Quotes)

Antes de enviar un pago, el sistema de SatsPath debe evaluar las tarifas de red, urgencia y métodos de la contraparte para decidir la ruta óptima.

```bash
# Consultar el perfil público de otro usuario
docker compose run --rm satspath-cli resolve bob@satspath.local

# Generar un "Quote" de pago por 50,000 sats (SatsPath decide la mejor ruta)
docker compose run --rm satspath-cli quote bob@satspath.local 50000

# Forzar una urgencia de pago rápida (altera la selección de red Lightning vs On-chain)
docker compose run --rm satspath-cli quote bob@satspath.local 250000 --urgency fast
```

---

## 4. Ejecución de Pagos y Swaps

El comando `pay` muestra la instrucción de pago. Si le agregas los flags experimentales, puedes simular la ejecución a través del `LightningExecutor` u orquestar Swaps Submarinos.

```bash
# Pagar a un destinatario y ver las instrucciones del QR (No mueve fondos)
docker compose run --rm satspath-cli pay bob@satspath.local 1000

# Simular la ejecución experimental del pago/swap en testnet
docker compose run --rm satspath-cli pay bob@satspath.local 100000 --testnet --experimental-swaps
```

---

## 5. El Flujo de Invitaciones (Invites & Claims)

Si le pagas a un usuario que **no** tiene un perfil registrado, SatsPath genera automáticamente un enlace de invitación.

```bash
# 1. Intentar pagarle a un usuario nuevo (Genera la invitación)
docker compose run --rm satspath-cli pay nuevo_usuario@satspath.local 50000

# El comando anterior te devolverá un enlace (ej: https://satspath.local/claim?alias_hash=...)
# 2. El nuevo usuario reclama su perfil usando el enlace
docker compose run --rm satspath-cli claim "https://satspath.local/claim?alias_hash=abc123def456&amount=50000"
```

---

## 6. Mantenimiento de Seguridad (Rotación de Llaves)

Si crees que tu llave de identidad fue comprometida, debes rotarla. SatsPath usa rotación criptográfica con pruebas `KeyRotation`.

```bash
# 1. Rotar las llaves de identidad (crea una nueva llave y adjunta la prueba)
docker compose run --rm satspath-cli wallet rotate

# 2. Validar el estado del perfil
# Deberías ver un aviso indicando que el perfil ha sido rotado recientemente
docker compose run --rm satspath-cli wallet show
```

---

## 7. ARK Bridge (Opcional)

Si necesitas utilizar el puente TypeScript de Ark para validación avanzada del cliente, debes levantar el perfil `bridge`.

```bash
# Iniciar el contenedor de ark-bridge
docker compose --profile bridge up -d ark-bridge

# Detener todos los servicios al terminar
docker compose --profile bridge down
```
