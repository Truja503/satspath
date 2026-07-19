//! P2P FFI — Hyperswarm / libp2p bridge (placeholder).

use crate::types::*;

/// Start the P2P bridge for sovereign profile resolution.
///
/// In production: starts an embedded P2P swarm (libp2p or Hyperswarm via iroh)
/// that listens for profile resolution requests over the DHT.
#[uniffi::export(async_runtime = "tokio")]
pub async fn start_p2p_bridge(_profile_path: String) -> Result<(), FfiError> {
    // TODO(Phase 5): Implement via P2pTransport trait + libp2p/iroh
    Err(FfiError::Other {
        reason: "P2P bridge not yet implemented — requires libp2p/iroh integration".into(),
    })
}

/// Stop the P2P bridge.
#[uniffi::export]
pub fn stop_p2p_bridge() {
    // TODO(Phase 5): Stop the running swarm
}