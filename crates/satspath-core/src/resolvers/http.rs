use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

use crate::resolver::ProfileResolver;
use crate::{Result, SatsPathError, SignedPaymentProfile};

/// Resolves a profile by making an HTTP GET request to the domain of the alias.
///
/// If the alias is `rodrigo@satspath.dev`, it fetches:
/// `https://satspath.dev/.well-known/satspath/rodrigo`
pub struct HttpResolver {
    client: Client,
}

impl HttpResolver {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl ProfileResolver for HttpResolver {
    async fn resolve_alias(&self, alias: &str) -> Result<SignedPaymentProfile> {
        let parts: Vec<&str> = alias.split('@').collect();
        if parts.len() != 2 {
            return Err(SatsPathError::AliasNotFound(alias.to_string()));
        }

        let username = parts[0];
        let domain = parts[1];

        // Must use HTTPS in production!
        let url = format!("https://{}/.well-known/satspath/{}", domain, username);

        let resp = self.client.get(&url).send().await.map_err(|e| {
            SatsPathError::NetworkError(format!("Failed to connect to {}: {}", domain, e))
        })?;

        if resp.status() == 404 {
            return Err(SatsPathError::AliasNotFound(alias.to_string()));
        }

        if !resp.status().is_success() {
            return Err(SatsPathError::NetworkError(format!(
                "HTTP error {} fetching profile",
                resp.status()
            )));
        }

        let profile: SignedPaymentProfile = resp.json().await.map_err(|e| {
            SatsPathError::SerializationError(format!("Failed to parse profile JSON: {}", e))
        })?;

        Ok(profile)
    }
}
