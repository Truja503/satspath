use async_trait::async_trait;

use crate::resolver::ProfileResolver;
use crate::{Result, SatsPathError, SignedPaymentProfile};

pub struct NostrResolver;

#[async_trait]
impl ProfileResolver for NostrResolver {
    async fn resolve_alias(&self, alias: &str) -> Result<SignedPaymentProfile> {
        Err(SatsPathError::AliasNotFound(format!(
            "Nostr resolver scaffold only: {alias}"
        )))
    }
}
