# SatsPath: Estado Actual (P2P) y Roadmap v0.2

Este documento resume el estado actual del proyecto (tras alcanzar el 100% de cumplimiento de la especificación **SatsPath Protocol v0.1**) y detalla las funcionalidades que el PDF original especifica como pendientes para futuras versiones ("Future behavior" / "Production" / "Do not build yet").

---

## 1. Lo que ya funciona (v0.1 Completada + P2P)

Hemos superado los requisitos básicos del Hackathon. Actualmente el sistema cuenta con:

- **100% Cumplimiento de la Spec v0.1**: Resolver, Router, perfiles firmados, rotación de llaves, verificación de firmas y estimación de comisiones.
- **Integración P2P Funcional**: Los perfiles pueden publicarse y resolverse de manera descentralizada utilizando Holepunch / Pear, eliminando la necesidad de un servidor central (`satspath wallet publish`).
- **Resolución Multi-backend**: Además del registro local y P2P, se soportan resolutores HTTP (`.well-known`) y Nostr (NIP-05).
- **CLI y Daemon Nativos y Dockerizados**: Entorno seguro y funcional tanto vía Docker como compilado en Rust de forma nativa (`satspath-cli`, `satspathd`).
- **Pruebas de Propiedad (Ownership Proofs)**: Sistema avanzado de mitigación de ataques que supera la especificación original.

---

## 2. Funcionalidades Faltantes (Roadmap v0.2 / Producción)

Al revisar la especificación de `SatsPath Protocol v0.1`, las siguientes funcionalidades fueron explícitamente marcadas como **"Future behavior"**, **"Production rule"** o **"Do not build yet"** (Sección 34). Esto es lo que falta construir para llevar el protocolo a producción real:

### A. Ejecución Real de Pagos (Payment Execution)

Actualmente el sistema genera **Intenciones de Pago** (URIs, códigos QR, facturas LN) y simula el éxito. Falta integrar la ejecución real:

- **Lightning Network Real (§18.1 y §34)**: Pagar facturas automáticamente desde un nodo Lightning integrado.
- **Transmisión On-chain Real (§34)**: Construcción de PSBTs (Partially Signed Bitcoin Transactions), firma de transacciones y _broadcasting_ a la mempool.
- **Implementación Real de Ark (§18.3 y §34)**: Conectarse a un ASP (Ark Service Provider) real para transferir VTXOs.
- **Ejecución de Pagos Divididos / Split Payments (§24)**: La estructura de datos ya está diseñada, pero el router aún no soporta la resolución recursiva y ejecución por lotes (_batching_) para múltiples receptores.
- **Custodial Escrow (§34)**: Depósito de garantía en custodia temporal (marcado como "Do not build yet").

### B. Privacidad y Criptografía

- **Silent Payments (§13 y §18.2)**: Para producción, la especificación exige no reutilizar direcciones estáticas. Se debe implementar generación de direcciones frescas o **Silent Payments** (BIP-352) para privacidad On-chain.
- **Cálculo Real de Tamaño de Transacción (§18.2)**: Actualmente se usan estimaciones estáticas (ej. 140 vB para P2WPKH). En producción, se debe armar el PSBT para calcular los _virtual bytes_ exactos.
- **Campos de Perfil Encriptados (§26 y §27)**: Para evitar fugas de privacidad en registros públicos, la especificación sugiere encriptación de campos usando secretos compartidos (ECDH).

### C. Seguridad Avanzada y Modelo de Amenazas (§26)

Las mitigaciones a futuro para vectores de ataque avanzados incluyen:

- **DNSSEC / BIP-353**: Validación criptográfica de resolución de nombres a través de DNS para mitigar registros de alias falsos.
- **Logs de Transparencia (Transparency Logs)**: Para auditar y evitar la manipulación de registros en servidores centrales/HTTP.
- **Políticas de Recuperación de Llaves (Multisig)**: Para mitigar la pérdida de la llave de identidad.
- **Invitaciones Firmadas y Verificación de Dominio (§27)**: Para mitigar ataques de enlaces de invitación maliciosos.
- **Validación de Pruebas y Reputación de Servidores Ark (§27)**: Para proteger a los usuarios de ASPs maliciosos.

### D. Experiencia de Usuario (Client / UX)

- **Billetera Móvil Completa (§34)**: El entregable actual es un Daemon y un CLI. Se requiere una aplicación móvil nativa (iOS/Android) para usuarios finales ("Do not build yet").
- **Soporte Completo para BOLT12 (§18.1)**: Obtención e interacción nativa con Ofertas BOLT12.

---

## Conclusión

El entregable actual es un **prototipo funcional** perfecto para el MVP y el hackathon, cumpliendo con la filosofía de _"Trust cryptographic signatures, not servers"_. El próximo gran salto (v0.2) será integrar los motores de firmas para mover fondos reales y añadir _Silent Payments_ para garantizar la privacidad.
