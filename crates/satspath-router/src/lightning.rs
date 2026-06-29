use satspath_core::PaymentMethod;

/// Check whether a payment method is a usable Lightning method.
pub fn is_lightning_available(method: &PaymentMethod) -> bool {
    match method {
        PaymentMethod::Lightning {
            lnurl,
            lightning_address,
            bolt12,
            ..
        } => lnurl.is_some() || lightning_address.is_some() || bolt12.is_some(),
        _ => false,
    }
}

/// Extract a Lightning Address from a method if present.
pub fn lightning_address(method: &PaymentMethod) -> Option<&str> {
    match method {
        PaymentMethod::Lightning {
            lightning_address, ..
        } => lightning_address.as_deref(),
        _ => None,
    }
}

/// Placeholder: fetch LNURL pay metadata. Not required for MVP.
///
/// In a full implementation this would perform the two-step LNURL-pay handshake
/// to obtain an invoice for the requested amount.
#[allow(dead_code)]
pub async fn fetch_lnurl_metadata(lnurl: &str) -> anyhow::Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let resp = client.get(lnurl).send().await?.json().await?;
    Ok(resp)
}

/// Estimate Lightning fee for small payments (rough heuristic).
pub fn estimate_lightning_fee_sats(amount_sats: u64) -> u64 {
    // Typically < 1 sat for small amounts; use 1 sat as a safe lower bound.
    std::cmp::max(1, amount_sats / 10_000)
}
