//! Resolver FFI implementation

use satspath_core::resolver::{ChainResolver, ProfileResolver};
use satspath_core::resolvers::{bip353::Bip353Resolver, http::HttpResolver, nostr::NostrResolver};
use satspath_core::{SignedPaymentProfile as CoreSignedPaymentProfile, SatsPathError};
use uniffi::deps::anyhow::Result;

// Use the generated FFI types from crate root
use crate::{
    SignedPaymentProfile as FfiSignedPaymentProfile,
    FfiError,
};

/// Resolver chain that combines all resolvers
#[derive(uniffi::Object)]
pub struct ResolverChain {
    pub inner: satspath_core::resolver::ChainResolver,
}

#[uniffi::export(async_runtime = "tokio")]
impl ResolverChain {
    #[uniffi::constructor]
    pub fn new() -> Self {
        let mut chain = satspath_core::resolver::ChainResolver::new();
        chain = chain.push(satspath_core::resolvers::bip353::Bip353Resolver::new());
        chain = chain.push(satspath_core::resolvers::http::HttpResolver::new());
        chain = chain.push(satspath_core::resolvers::nostr::NostrResolver::new());
        // P2P resolver would be added here when available
        Self { inner: chain }
    }

    pub async fn resolve_alias(&self, alias: String) -> Result<crate::SignedPaymentProfile, crate::FfiError> {
        self.inner.resolve_alias(&alias).await
            .map(|p| p.into())
            .map_err(|e| crate::FfiError::Other(e.to_string()))
    }
}

impl Default for ResolverChain {
    fn default() -> Self {
        Self::new()
    }
}

// Implement the generated ProfileResolver trait for ResolverChain
impl crate::ProfileResolver for ResolverChain {
    async fn resolveAlias(&self, alias: &str) -> crate::SignedPaymentProfile {
        self.inner.resolve_alias(alias).await
            .map(|p| p.into())
            .unwrap_or_else(|e| panic!("Resolver error: {}", e))
    }
}