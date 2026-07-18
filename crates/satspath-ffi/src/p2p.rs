//! P2P FFI implementation (placeholder for Hyperswarm integration)

use satspath_core::SatsPathError;
use uniffi::deps::anyhow::Result;

/// Start the P2P bridge (Hyperswarm via hyperdriver)
#[uniffi::export(async_runtime = "tokio")]
pub async fn start_p2p_bridge_ffi(profile_path: String) -> Result<(), crate::r#FfiError> {
    // In production: start embedded Hyperswarm via hyperdriver
    // The profile_path points to the encrypted profile storage
    // This runs the swarm in background on mobile
    eprintln!("Starting P2P bridge with profile: {}", profile_path);
    Err(crate::r#FfiError::Other("P2P bridge not yet implemented - requires hyperdriver integration".into()))
}

/// Stop the P2P bridge
#[uniffi::export]
pub fn stop_p2p_bridge_ffi() {
    // In production: stop the Hyperswarm swarm
    eprintln!("Stopping P2P bridge");
}