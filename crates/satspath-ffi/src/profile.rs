//! Profile FFI — save/load profiles (platform-specific storage).

use crate::types::*;

/// Save a profile to local storage.
///
/// In production, this delegates to platform-specific encrypted storage:
/// - Android: SQLCipher / KeyStore
/// - iOS: Keychain
/// - Desktop: SQLCipher
/// - WASM: IndexedDB (encrypted)
#[uniffi::export(async_runtime = "tokio")]
pub async fn save_profile(_profile: FfiSignedPaymentProfile) -> Result<(), FfiError> {
    // TODO(Phase 4): Implement via SecureStorage trait
    Ok(())
}

/// Load a profile from local storage by alias.
#[uniffi::export(async_runtime = "tokio")]
pub async fn load_profile(_alias: String) -> Result<Option<FfiSignedPaymentProfile>, FfiError> {
    // TODO(Phase 4): Implement via SecureStorage trait
    Ok(None)
}