use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::crypto::{check_profile_expiry, verify_signed_profile};
use crate::peer_registry::canonicalize_identifier;
use crate::resolver::ProfileResolver;
use crate::{Result, SatsPathError, SignedPaymentProfile};

const SATSPATH_PROFILE_KIND: u64 = 30_078;
const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.primal.net",
];
const NOSTR_TIMEOUT: Duration = Duration::from_secs(8);

/// Resolver for SatsPath profiles announced over Nostr.
///
/// Discovery is transport-neutral:
/// 1. Resolve `user@domain` through NIP-05 (`/.well-known/nostr.json?name=user`).
/// 2. Use the returned pubkey and relay hints.
/// 3. Query relay events for kind 30078 and `d=satspath-profile:<canonical_id>`.
/// 4. Parse event content as `SignedPaymentProfile`.
/// 5. Verify the SatsPath profile signature and expiry before returning it.
///
/// Nostr event signatures identify the Nostr author. They do not replace the
/// SatsPath profile signature, which is still the protocol authority.
pub struct NostrResolver {
    client: reqwest::Client,
    fallback_relays: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Nip05Document {
    names: HashMap<String, String>,
    #[serde(default)]
    relays: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
struct Nip05Resolution {
    pubkey: String,
    relays: Vec<String>,
}

impl Default for NostrResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl NostrResolver {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(6))
                .build()
                .unwrap_or_default(),
            fallback_relays: env_relays().unwrap_or_else(|| {
                DEFAULT_RELAYS
                    .iter()
                    .map(|relay| relay.to_string())
                    .collect()
            }),
        }
    }

    pub fn with_relays(relays: Vec<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(6))
                .build()
                .unwrap_or_default(),
            fallback_relays: relays,
        }
    }

    async fn resolve_nip05(&self, alias: &str) -> Result<Nip05Resolution> {
        let canonical = canonicalize_identifier(alias);
        let (name, domain) = canonical
            .split_once('@')
            .ok_or_else(|| SatsPathError::AliasNotFound(alias.to_string()))?;
        let url = format!("https://{domain}/.well-known/nostr.json?name={name}");
        let document: Nip05Document = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SatsPathError::NetworkError(format!("NIP-05 fetch failed: {e}")))?
            .error_for_status()
            .map_err(|e| SatsPathError::NetworkError(format!("NIP-05 HTTP error: {e}")))?
            .json()
            .await
            .map_err(|e| SatsPathError::SerializationError(format!("NIP-05 JSON: {e}")))?;

        let pubkey = document
            .names
            .get(name)
            .or_else(|| document.names.get(&name.to_ascii_lowercase()))
            .cloned()
            .ok_or_else(|| SatsPathError::AliasNotFound(alias.to_string()))?;
        validate_nostr_pubkey(&pubkey)?;

        let mut relays = document.relays.get(&pubkey).cloned().unwrap_or_default();
        if relays.is_empty() {
            relays = self.fallback_relays.clone();
        }
        relays.retain(|relay| relay.starts_with("wss://") || relay.starts_with("ws://"));
        relays.truncate(8);
        if relays.is_empty() {
            return Err(SatsPathError::NetworkError(
                "no usable Nostr relays for NIP-05 identity".into(),
            ));
        }

        Ok(Nip05Resolution { pubkey, relays })
    }

    async fn query_relay(
        &self,
        relay: &str,
        pubkey: &str,
        alias: &str,
    ) -> Result<SignedPaymentProfile> {
        let canonical = canonicalize_identifier(alias);
        let d_tag = format!("satspath-profile:{canonical}");
        let sub_id = format!("satspath-{}", chrono::Utc::now().timestamp_millis());
        let req = json!([
            "REQ",
            sub_id,
            {
                "authors": [pubkey],
                "kinds": [SATSPATH_PROFILE_KIND],
                "#d": [d_tag],
                "limit": 5
            }
        ]);

        let relay_result = timeout(NOSTR_TIMEOUT, async {
            let (mut ws, _) = connect_async(relay)
                .await
                .map_err(|e| SatsPathError::NetworkError(format!("Nostr relay connect: {e}")))?;
            ws.send(Message::Text(req.to_string()))
                .await
                .map_err(|e| SatsPathError::NetworkError(format!("Nostr relay send: {e}")))?;

            while let Some(msg) = ws.next().await {
                let msg =
                    msg.map_err(|e| SatsPathError::NetworkError(format!("Nostr relay read: {e}")))?;
                let Message::Text(text) = msg else {
                    continue;
                };
                if let Some(signed) = signed_profile_from_event(&text, &sub_id, pubkey, alias)? {
                    return Ok(signed);
                }
            }

            Err(SatsPathError::AliasNotFound(alias.to_string()))
        })
        .await
        .map_err(|_| SatsPathError::NetworkError(format!("Nostr relay timeout: {relay}")))?;

        relay_result
    }
}

#[async_trait]
impl ProfileResolver for NostrResolver {
    async fn resolve_alias(&self, alias: &str) -> Result<SignedPaymentProfile> {
        let nip05 = self.resolve_nip05(alias).await?;
        let mut last_error = None;

        for relay in &nip05.relays {
            match self.query_relay(relay, &nip05.pubkey, alias).await {
                Ok(signed) => return Ok(signed),
                Err(SatsPathError::AliasNotFound(_)) => {}
                Err(e) => last_error = Some(e),
            }
        }

        match last_error {
            Some(e) => Err(e),
            None => Err(SatsPathError::AliasNotFound(alias.to_string())),
        }
    }
}

fn signed_profile_from_event(
    raw: &str,
    sub_id: &str,
    expected_pubkey: &str,
    alias: &str,
) -> Result<Option<SignedPaymentProfile>> {
    let value: Value = serde_json::from_str(raw)?;
    let Some(array) = value.as_array() else {
        return Ok(None);
    };
    if array.first().and_then(Value::as_str) != Some("EVENT") {
        return Ok(None);
    }
    if array.get(1).and_then(Value::as_str) != Some(sub_id) {
        return Ok(None);
    }
    let Some(event) = array.get(2) else {
        return Ok(None);
    };
    if event.get("kind").and_then(Value::as_u64) != Some(SATSPATH_PROFILE_KIND) {
        return Ok(None);
    }
    if event.get("pubkey").and_then(Value::as_str) != Some(expected_pubkey) {
        return Ok(None);
    }

    let wanted = canonicalize_identifier(alias);
    let d_tag = format!("satspath-profile:{wanted}");
    if !event_has_tag(event, "d", &d_tag) {
        return Ok(None);
    }

    let content = event
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| SatsPathError::SerializationError("Nostr event missing content".into()))?;
    let signed: SignedPaymentProfile = serde_json::from_str(content)?;
    let got = canonicalize_identifier(&signed.profile.alias);
    if got != wanted {
        return Err(SatsPathError::InvalidRoute(format!(
            "Nostr profile alias mismatch: expected {wanted}, got {got}"
        )));
    }
    if !verify_signed_profile(&signed)? {
        return Err(SatsPathError::InvalidSignature);
    }
    check_profile_expiry(&signed.profile)?;
    Ok(Some(signed))
}

fn event_has_tag(event: &Value, tag_name: &str, tag_value: &str) -> bool {
    event
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter().any(|tag| {
                let Some(items) = tag.as_array() else {
                    return false;
                };
                items.first().and_then(Value::as_str) == Some(tag_name)
                    && items.get(1).and_then(Value::as_str) == Some(tag_value)
            })
        })
        .unwrap_or(false)
}

fn validate_nostr_pubkey(pubkey: &str) -> Result<()> {
    if pubkey.len() == 64 && pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(SatsPathError::InvalidPublicKey(
            "NIP-05 pubkey must be 32-byte hex".into(),
        ))
    }
}

fn env_relays() -> Option<Vec<String>> {
    std::env::var("SATSPATH_NOSTR_RELAYS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|relay| !relay.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|relays| !relays.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{generate_identity_keypair, sign_profile};
    use crate::{PaymentMethod, PaymentProfile};

    fn signed(alias: &str) -> SignedPaymentProfile {
        let kp = generate_identity_keypair();
        let profile = PaymentProfile {
            alias: alias.to_string(),
            identity_pubkey: hex::encode(kp.public_key.serialize()),
            methods: vec![PaymentMethod::Lightning {
                label: "Lightning".into(),
                lightning_address: Some(alias.to_string()),
                lnurl: None,
                bolt12: None,
                receiver_pubkey: None,
            }],
            updated_at: 1_782_810_000,
            expires_at: None,
            method_verifications: Vec::new(),
        };
        sign_profile(profile, &kp.secret_key).unwrap()
    }

    #[test]
    fn validates_nostr_pubkey_shape() {
        assert!(validate_nostr_pubkey(&"a".repeat(64)).is_ok());
        assert!(validate_nostr_pubkey("npub1abc").is_err());
    }

    #[test]
    fn parses_signed_profile_event_content() {
        let profile = signed("alice@example.com");
        let content = serde_json::to_string(&profile).unwrap();
        let raw = json!([
            "EVENT",
            "sub",
            {
                "kind": SATSPATH_PROFILE_KIND,
                "pubkey": "a".repeat(64),
                "content": content,
                "tags": [["d", "satspath-profile:alice@example.com"]]
            }
        ])
        .to_string();

        let parsed = signed_profile_from_event(&raw, "sub", &"a".repeat(64), "alice@example.com")
            .unwrap()
            .unwrap();
        assert_eq!(parsed.profile.alias, "alice@example.com");
    }

    #[test]
    fn rejects_event_for_wrong_alias() {
        let profile = signed("bob@example.com");
        let content = serde_json::to_string(&profile).unwrap();
        let raw = json!([
            "EVENT",
            "sub",
            {
                "kind": SATSPATH_PROFILE_KIND,
                "pubkey": "a".repeat(64),
                "tags": [["d", "satspath-profile:alice@example.com"]],
                "content": content
            }
        ])
        .to_string();

        assert!(
            signed_profile_from_event(&raw, "sub", &"a".repeat(64), "alice@example.com").is_err()
        );
    }
}
