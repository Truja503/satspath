use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use satspath_core::errors::{Result, SatsPathError};

#[derive(Debug, Deserialize)]
pub struct LnurlPayResponse {
    pub callback: String,
    #[serde(rename = "maxSendable")]
    pub max_sendable: u64,
    #[serde(rename = "minSendable")]
    pub min_sendable: u64,
    pub metadata: String,
    pub tag: String,
}

#[derive(Debug, Deserialize)]
pub struct LnurlInvoiceResponse {
    pub pr: String, // BOLT11 invoice
    pub routes: Vec<serde_json::Value>,
}

/// Fetch LNURL-Pay metadata from a Lightning Address (user@domain.com)
pub async fn fetch_lnurl_metadata(address: &str) -> Result<LnurlPayResponse> {
    let parts: Vec<&str> = address.split('@').collect();
    if parts.len() != 2 {
        return Err(SatsPathError::InvalidRoute(format!(
            "Invalid Lightning Address: {}",
            address
        )));
    }

    let username = parts[0];
    let domain = parts[1];
    let url = format!("https://{}/.well-known/lnurlp/{}", domain, username);

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let resp = client.get(&url).send().await.map_err(|e| {
        SatsPathError::NetworkError(format!("LNURL fetch failed: {}", e))
    })?;

    if !resp.status().is_success() {
        return Err(SatsPathError::NetworkError(format!(
            "HTTP {} fetching LNURL",
            resp.status()
        )));
    }

    let data: LnurlPayResponse = resp.json().await.map_err(|e| {
        SatsPathError::SerializationError(format!("Invalid LNURL response: {}", e))
    })?;

    if data.tag != "payRequest" {
        return Err(SatsPathError::InvalidRoute(
            "LNURL response is not a payRequest".to_string(),
        ));
    }

    Ok(data)
}

/// Fetch a BOLT11 invoice from the LNURL callback using the requested amount (in millisatoshis)
pub async fn fetch_lnurl_invoice(callback: &str, msats: u64) -> Result<String> {
    let url = format!("{}?amount={}", callback, msats);

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let resp = client.get(&url).send().await.map_err(|e| {
        SatsPathError::NetworkError(format!("Invoice fetch failed: {}", e))
    })?;

    if !resp.status().is_success() {
        return Err(SatsPathError::NetworkError(format!(
            "HTTP {} fetching invoice",
            resp.status()
        )));
    }

    let data: LnurlInvoiceResponse = resp.json().await.map_err(|e| {
        SatsPathError::SerializationError(format!("Invalid invoice response: {}", e))
    })?;

    Ok(data.pr)
}
