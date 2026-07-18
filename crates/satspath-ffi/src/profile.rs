//! Profile FFI implementation (internal - no direct exports)

use satspath_core::{SignedPaymentProfile as CoreSignedPaymentProfile, SatsPathError};

/// Save a profile to local storage (platform-specific implementation needed)
pub async fn save_profile_ffi(_profile: CoreSignedPaymentProfile) -> Result<(), SatsPathError> {
    // In production, save to platform-specific encrypted storage
    // Android: SQLCipher/KeyStore, iOS: Keychain, Desktop: SQLCipher
    // WASM: IndexedDB / localStorage (encrypted)
    Ok(())
}

/// Load a profile from local storage
pub async fn load_profile_ffi(_alias: String) -> Result<Option<CoreSignedPaymentProfile>, SatsPathError> {
    // Platform-specific storage implementation required
    Ok(None)
}

/// Verify a signed profile's signature
pub fn verify_profile_ffi(profile: crate::r#SignedPaymentProfile) -> bool {
    satspath_core::crypto::verify_signed_profile(&profile.into()).unwrap_or(false)
}