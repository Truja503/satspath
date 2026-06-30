use async_trait::async_trait;

use crate::{Result, SatsPathError, SignedPaymentProfile};
use crate::privacy::canonical_identifier;

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

impl Default for ChainResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProfileResolver for ChainResolver {
    async fn resolve_alias(&self, alias: &str) -> Result<SignedPaymentProfile> {
        let requested_canonical = canonical_identifier(alias);
        for resolver in &self.resolvers {
            match resolver.resolve_alias(alias).await {
                Ok(profile) => {
                    // SEC-02: Profile Substitution Attack mitigation
                    let returned_canonical = canonical_identifier(&profile.profile.alias);
                    if returned_canonical != requested_canonical {
                        // A resolver returned a valid profile but for a DIFFERENT user.
                        // Fail hard to prevent funds from being routed to the attacker.
                        return Err(SatsPathError::AliasNotFound(format!(
                            "security failure: requested {}, but got profile for {}",
                            requested_canonical, returned_canonical
                        )));
                    }
                    return Ok(profile);
                }
                Err(SatsPathError::AliasNotFound(_)) => continue,
                Err(SatsPathError::NetworkError(_)) => continue,
                Err(e) => return Err(e),
            }
        }

        Err(SatsPathError::AliasNotFound(alias.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{generate_identity_keypair, sign_profile};
    use crate::profile::{PaymentMethod, PaymentProfile};

    struct MockResolver {
        should_succeed: bool,
        returned_alias: Option<String>,
    }

    #[async_trait]
    impl ProfileResolver for MockResolver {
        async fn resolve_alias(&self, alias: &str) -> Result<SignedPaymentProfile> {
            if self.should_succeed {
                let kp = generate_identity_keypair();
                let pubkey_hex = hex::encode(kp.public_key.serialize());
                let profile_alias = self.returned_alias.clone().unwrap_or_else(|| alias.to_string());
                let profile = PaymentProfile {
                    alias: profile_alias.clone(),
                    identity_pubkey: pubkey_hex,
                    methods: vec![PaymentMethod::Lightning {
                        label: "LN".into(),
                        lightning_address: Some(profile_alias),
                        lnurl: None,
                        bolt12: None,
                        receiver_pubkey: None,
                    }],
                    updated_at: 1_700_000_000,
                    expires_at: None,
                    method_verifications: Vec::new(),
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
                returned_alias: None,
            })
            .push(MockResolver {
                should_succeed: true,
                returned_alias: None,
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
                returned_alias: None,
            })
            .push(MockResolver {
                should_succeed: false,
                returned_alias: None,
            });

        let res = chain.resolve_alias("test@domain.com").await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn chain_resolver_rejects_substituted_alias() {
        // We ask the mock resolver to succeed, which always returns a profile
        // for "test@domain.com".
        let chain = ChainResolver::new().push(MockResolver {
            should_succeed: true,
            returned_alias: Some("test@domain.com".to_string()),
        });

        // But we request "alice@domain.com".
        // The resolver will return "test@domain.com", which should trigger
        // the SEC-02 Profile Substitution Attack mitigation.
        let res = chain.resolve_alias("alice@domain.com").await;
        
        assert!(res.is_err());
        let err = res.unwrap_err().to_string();
        assert!(err.contains("security failure"), "Expected security failure message, got: {}", err);
        assert!(err.contains("alice@domain.com"));
        assert!(err.contains("test@domain.com"));
    }
}
