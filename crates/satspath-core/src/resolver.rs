use async_trait::async_trait;

use crate::{Result, SatsPathError, SignedPaymentProfile};

/// A resolver capable of resolving a SatsPath alias to a `SignedPaymentProfile`.
///
/// Implementations could be local (e.g. `LocalRegistryResolver`) or remote
/// (e.g. `HttpResolver` for NIP-05 style public resolution).
#[async_trait]
pub trait ProfileResolver {
    /// Attempt to resolve the alias to a signed profile.
    ///
    /// # Returns
    /// - `Ok(SignedPaymentProfile)` if the alias is found.
    /// - `Err(SatsPathError::AliasNotFound)` if the alias is explicitly not found.
    /// - `Err(SatsPathError::NetworkError)` (or similar) on fetch failure.
    async fn resolve_alias(&self, alias: &str) -> Result<SignedPaymentProfile>;
}

/// A composite resolver that queries a chain of resolvers in order.
pub struct ChainResolver {
    resolvers: Vec<Box<dyn ProfileResolver + Send + Sync>>,
}

impl ChainResolver {
    pub fn new() -> Self {
        Self {
            resolvers: Vec::new(),
        }
    }

    pub fn push<R: ProfileResolver + Send + Sync + 'static>(mut self, resolver: R) -> Self {
        self.resolvers.push(Box::new(resolver));
        self
    }
}

#[async_trait]
impl ProfileResolver for ChainResolver {
    async fn resolve_alias(&self, alias: &str) -> Result<SignedPaymentProfile> {
        let mut last_error = None;

        for resolver in &self.resolvers {
            match resolver.resolve_alias(alias).await {
                Ok(profile) => return Ok(profile),
                Err(SatsPathError::AliasNotFound(_)) => continue,
                Err(e) => {
                    // Save the error, but keep trying the next resolver
                    last_error = Some(e);
                    continue;
                }
            }
        }

        if let Some(e) = last_error {
            Err(e)
        } else {
            Err(SatsPathError::AliasNotFound(alias.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{generate_identity_keypair, sign_profile};
    use crate::profile::{PaymentMethod, PaymentProfile};

    struct MockResolver {
        should_succeed: bool,
    }

    #[async_trait]
    impl ProfileResolver for MockResolver {
        async fn resolve_alias(&self, alias: &str) -> Result<SignedPaymentProfile> {
            if self.should_succeed {
                let kp = generate_identity_keypair();
                let pubkey_hex = hex::encode(kp.public_key.serialize());
                let profile = PaymentProfile {
                    alias: alias.to_string(),
                    identity_pubkey: pubkey_hex,
                    methods: vec![PaymentMethod::Lightning {
                        label: "LN".into(),
                        lnurl: None,
                        lightning_address: Some(alias.to_string()),
                        bolt12: None,
                    }],
                    updated_at: 1_700_000_000,
                };
                Ok(sign_profile(profile, &kp.secret_key).unwrap())
            } else {
                Err(SatsPathError::AliasNotFound(alias.to_string()))
            }
        }
    }

    #[tokio::test]
    async fn chain_resolver_success() {
        let chain = ChainResolver::new()
            .push(MockResolver {
                should_succeed: false,
            })
            .push(MockResolver {
                should_succeed: true,
            });

        let res = chain.resolve_alias("test@domain.com").await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap().profile.alias, "test@domain.com");
    }

    #[tokio::test]
    async fn chain_resolver_failure() {
        let chain = ChainResolver::new()
            .push(MockResolver {
                should_succeed: false,
            })
            .push(MockResolver {
                should_succeed: false,
            });

        let res = chain.resolve_alias("test@domain.com").await;
        assert!(res.is_err());
    }
}
