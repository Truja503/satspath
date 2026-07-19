//! Resolver FFI — exposes the resolver chain to foreign platforms.

use crate::types::*;
use satspath_core::resolver::{ChainResolver, ProfileResolver};

/// Resolver chain that combines all resolvers (BIP-353, HTTP, Nostr).
/// Exposed to foreign platforms via UniFFI.
#[derive(uniffi::Object)]
pub struct FfiResolverChain {
    inner: ChainResolver,
}

#[uniffi::export(async_runtime = "tokio")]
impl FfiResolverChain {
    /// Create a new resolver chain with all built-in resolvers.
    #[uniffi::constructor]
    pub fn new() -> Self {
        let chain = ChainResolver::new()
            .push(satspath_core::resolvers::bip353::Bip353Resolver::new())
            .push(satspath_core::resolvers::http::HttpResolver::new())
            .push(satspath_core::resolvers::nostr::NostrResolver::new());
        // P2P resolver added when available
        Self { inner: chain }
    }

    /// Resolve an alias to a signed payment profile.
    pub async fn resolve_alias(&self, alias: String) -> Result<FfiSignedPaymentProfile, FfiError> {
        self.inner
            .resolve_alias(&alias)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }
}

impl FfiResolverChain {
    /// Get a reference to the inner ChainResolver (for internal use).
    pub(crate) fn inner(&self) -> &ChainResolver {
        &self.inner
    }
}