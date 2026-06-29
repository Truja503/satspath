use async_trait::async_trait;

use crate::resolver::ProfileResolver;
use crate::{Result, SatsPathError, SignedPaymentProfile};

pub struct Bip353Resolver {
    dnssec_required: bool,
}

impl Bip353Resolver {
    pub fn new() -> Self {
        Self {
            dnssec_required: true,
        }
    }

    pub fn dnssec_required(&self) -> bool {
        self.dnssec_required
    }
}

#[async_trait]
impl ProfileResolver for Bip353Resolver {
    async fn resolve_alias(&self, alias: &str) -> Result<SignedPaymentProfile> {
        if !alias.starts_with('₿') {
            return Err(SatsPathError::AliasNotFound(alias.to_string()));
        }
        Err(SatsPathError::InvalidRoute(
            "BIP-353 DNSSEC validation is not implemented; failing closed".into(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bip321PaymentUri {
    pub uri: String,
}
