use serde::Deserialize;

use satspath_core::PaymentMethod;

pub fn is_lightning_available(method: &PaymentMethod) -> bool {
    match method {
        PaymentMethod::Lightning { lnurl, lightning_address, bolt12, .. } => {
            lnurl.is_some() || lightning_address.is_some() || bolt12.is_some()
        }
        _ => false,
    }
}

pub fn lightning_address(method: &PaymentMethod) -> Option<&str> {
    match method {
        PaymentMethod::Lightning { lightning_address, .. } => lightning_address.as_deref(),
        _ => None,
    }
}

pub fn estimate_lightning_fee_sats(amount_sats: u64) -> u64 {
    std::cmp::max(1, amount_sats / 10_000)
}

// ─── LNURL-pay two-step protocol ─────────────────────────────────────────────

/// Response from step 1: GET https://domain/.well-known/lnurlp/<user>
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LnurlPayMetadata {
    pub callback: String,
    pub min_sendable: u64, // millisatoshis
    pub max_sendable: u64, // millisatoshis
    pub tag: String,
    #[serde(default)]
    pub comment_allowed: u64,
}

/// Response from step 2: GET {callback}?amount=<msats>
#[derive(Debug, Deserialize)]
pub struct LnurlInvoiceResponse {
    pub pr: String,
}

/// Step 1: resolve a Lightning Address to LNURL-pay metadata.
/// `user@domain` → GET `https://domain/.well-known/lnurlp/user`
pub async fn fetch_lnurl_metadata(lightning_address: &str) -> anyhow::Result<LnurlPayMetadata> {
    let parts: Vec<&str> = lightning_address.splitn(2, '@').collect();
    if parts.len() != 2 {
        anyhow::bail!("invalid Lightning Address: {}", lightning_address);
    }
    let url = format!(
        "https://{}/.well-known/lnurlp/{}",
        parts[1].trim(),
        parts[0].trim()
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let meta = client.get(&url).send().await?.json::<LnurlPayMetadata>().await?;
    if meta.tag != "payRequest" {
        anyhow::bail!("unexpected LNURL tag: {}", meta.tag);
    }
    Ok(meta)
}

/// Step 2: call the callback to get a real BOLT11 invoice.
pub async fn fetch_invoice(
    meta: &LnurlPayMetadata,
    amount_sats: u64,
    comment: Option<&str>,
) -> anyhow::Result<String> {
    let amount_msats = amount_sats * 1_000;
    if amount_msats < meta.min_sendable {
        anyhow::bail!(
            "amount {} sats ({} msats) below minimum {} msats",
            amount_sats, amount_msats, meta.min_sendable
        );
    }
    if amount_msats > meta.max_sendable {
        anyhow::bail!(
            "amount {} sats ({} msats) exceeds maximum {} msats",
            amount_sats, amount_msats, meta.max_sendable
        );
    }
    let mut url = format!("{}?amount={}", meta.callback, amount_msats);
    if let Some(c) = comment {
        if meta.comment_allowed > 0 {
            let trimmed = &c[..c.len().min(meta.comment_allowed as usize)];
            url.push_str(&format!("&comment={}", urlencoding::encode(trimmed)));
        }
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let resp = client.get(&url).send().await?.json::<LnurlInvoiceResponse>().await?;
    if resp.pr.is_empty() {
        anyhow::bail!("received empty invoice from LNURL callback");
    }
    Ok(resp.pr)
}
