use serde::Deserialize;

use satspath_core::PaymentMethod;

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

pub fn lightning_address(method: &PaymentMethod) -> Option<&str> {
    match method {
        PaymentMethod::Lightning {
            lightning_address, ..
        } => lightning_address.as_deref(),
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
    let meta = client
        .get(&url)
        .send()
        .await?
        .json::<LnurlPayMetadata>()
        .await?;
    if meta.tag != "payRequest" {
        anyhow::bail!("unexpected LNURL tag: {}", meta.tag);
    }
    Ok(meta)
}

/// Step 2: call the callback to get a real BOLT11 invoice, then verify the amount.
pub async fn fetch_invoice(
    meta: &LnurlPayMetadata,
    amount_sats: u64,
    comment: Option<&str>,
) -> anyhow::Result<String> {
    let amount_msats = amount_sats * 1_000;
    if amount_msats < meta.min_sendable {
        anyhow::bail!(
            "amount {} sats ({} msats) below minimum {} msats",
            amount_sats,
            amount_msats,
            meta.min_sendable
        );
    }
    if amount_msats > meta.max_sendable {
        anyhow::bail!(
            "amount {} sats ({} msats) exceeds maximum {} msats",
            amount_sats,
            amount_msats,
            meta.max_sendable
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
    let resp = client
        .get(&url)
        .send()
        .await?
        .json::<LnurlInvoiceResponse>()
        .await?;
    if resp.pr.is_empty() {
        anyhow::bail!("received empty invoice from LNURL callback");
    }
    verify_invoice_amount(&resp.pr, amount_sats)?;
    Ok(resp.pr)
}

// ─── BOLT11 invoice amount verification ───────────────────────────────────────

/// Parse the satoshi amount encoded in a BOLT11 invoice's human-readable part (HRP).
///
/// BOLT11 HRP format: `ln<network>[<amount><multiplier>]`
/// Networks: `bc` (mainnet), `tb` (testnet), `bcrt` (regtest)
/// Multipliers: m (milli), u (micro), n (nano), p (pico)
///
/// Returns `None` if the invoice carries no amount (amount-less invoice).
pub fn parse_bolt11_amount_sats(invoice: &str) -> Option<u64> {
    let inv = invoice.to_lowercase();
    // bech32 separator: the last '1' in the string
    let sep = inv.rfind('1')?;
    let hrp = &inv[..sep];

    // Strip the network prefix to get the optional amount string
    let amount_str = hrp
        .strip_prefix("lnbc") // mainnet
        .or_else(|| hrp.strip_prefix("lntb")) // testnet
        .or_else(|| hrp.strip_prefix("lnbcrt")) // regtest
        .or_else(|| hrp.strip_prefix("lnsb")) // signet
        .or_else(|| hrp.strip_prefix("lntbs"))?; // testnet4

    if amount_str.is_empty() {
        return None; // amount-less invoice
    }

    // Last char may be a multiplier letter
    let last_char = amount_str.chars().last()?;
    let (digits, multiplier) = if last_char.is_alphabetic() {
        (&amount_str[..amount_str.len() - 1], Some(last_char))
    } else {
        (amount_str, None)
    };

    if digits.is_empty() {
        return None;
    }

    let amount_val: u64 = digits.parse().ok()?;

    // BOLT11 amounts are BTC scaled by the multiplier.
    // 1 BTC = 100_000_000 sats.
    match multiplier {
        None => amount_val.checked_mul(100_000_000),  // raw BTC
        Some('m') => amount_val.checked_mul(100_000), // milli-BTC = 1e5 sats
        Some('u') => amount_val.checked_mul(100),     // micro-BTC = 100 sats
        Some('n') => {
            // nano-BTC = 0.1 sats; must be divisible by 10 for whole sats
            if amount_val % 10 != 0 {
                return None;
            }
            Some(amount_val / 10)
        }
        Some('p') => {
            // pico-BTC = 0.0001 sats; must be divisible by 10_000
            if amount_val % 10_000 != 0 {
                return None;
            }
            Some(amount_val / 10_000)
        }
        _ => None,
    }
}

/// Verify that a BOLT11 invoice encodes exactly `expected_sats`.
///
/// Aborts if:
/// - The invoice carries no amount (amount-less invoices are not accepted)
/// - The encoded amount does not match `expected_sats`
///
/// Note: expiry verification requires bech32 data field decoding and is tracked
/// as Engine v1 work. Until implemented, expiry is not checked here.
pub fn verify_invoice_amount(invoice: &str, expected_sats: u64) -> anyhow::Result<()> {
    match parse_bolt11_amount_sats(invoice) {
        None => anyhow::bail!(
            "invoice carries no amount (amount-less BOLT11 not accepted in Engine v0). \
             Expected {} sats.",
            expected_sats
        ),
        Some(invoice_sats) if invoice_sats != expected_sats => anyhow::bail!(
            "invoice amount mismatch: invoice encodes {} sats, expected {} sats. \
             Refusing to display a mismatched invoice.",
            invoice_sats,
            expected_sats
        ),
        Some(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: build a fake BOLT11 string with the given HRP prefix and garbage data.
    // Only the HRP matters for amount parsing.
    fn fake_invoice(hrp: &str) -> String {
        format!("{}1pvjluezqpzry9x8gl4kzd8m7nt3g5p", hrp)
    }

    #[test]
    fn invoice_amount_matches_requested() {
        // lnbc10u = 10 micro-BTC = 10 × 100 = 1000 sats
        let inv = fake_invoice("lnbc10u");
        assert!(verify_invoice_amount(&inv, 1000).is_ok());
    }

    #[test]
    fn invoice_amount_mismatch_fails() {
        // lnbc10u = 1000 sats, but we expected 500
        let inv = fake_invoice("lnbc10u");
        let err = verify_invoice_amount(&inv, 500).unwrap_err();
        assert!(err.to_string().contains("mismatch"), "got: {err}");
    }

    #[test]
    fn zero_or_missing_amount_fails() {
        // Invoice with no amount in HRP (lnbc only, no digits)
        let inv = fake_invoice("lnbc");
        let err = verify_invoice_amount(&inv, 1000).unwrap_err();
        assert!(err.to_string().contains("no amount"), "got: {err}");
    }

    #[test]
    #[ignore = "BOLT11 expiry requires bech32 data-field decode — Engine v1 TODO"]
    fn expired_invoice_fails() {
        // When expiry parsing is implemented, this test should verify that an invoice
        // with a timestamp + expiry that has passed is rejected by verify_invoice_amount
        // (or a separate verify_invoice_not_expired() call).
        todo!("implement bech32 data field decode for expiry tag (tag 6)")
    }

    #[test]
    fn parse_bolt11_mainnet_micro() {
        // lnbc10u = 10 μBTC = 1000 sats
        assert_eq!(
            parse_bolt11_amount_sats(&fake_invoice("lnbc10u")),
            Some(1000)
        );
    }

    #[test]
    fn parse_bolt11_milli() {
        // lnbc21m = 21 mBTC = 2_100_000 sats
        assert_eq!(
            parse_bolt11_amount_sats(&fake_invoice("lnbc21m")),
            Some(2_100_000)
        );
    }

    #[test]
    fn parse_bolt11_no_amount() {
        // lnbc (no digits) = amount-less invoice
        assert_eq!(parse_bolt11_amount_sats(&fake_invoice("lnbc")), None);
    }

    #[test]
    fn parse_bolt11_testnet() {
        // lntb500u = 500 μBTC = 50_000 sats
        assert_eq!(
            parse_bolt11_amount_sats(&fake_invoice("lntb500u")),
            Some(50_000)
        );
    }

    #[test]
    fn parse_bolt11_raw_btc() {
        // lnbc1 (no multiplier) = 1 BTC = 100_000_000 sats
        // Note: the "1" is now ambiguous with bech32 separator — depends on actual invoice
        // This tests the path where digits but no multiplier are present.
        // To avoid separator ambiguity, use a multi-digit amount.
        assert_eq!(
            parse_bolt11_amount_sats(&fake_invoice("lnbc2")),
            Some(200_000_000)
        );
    }

    #[test]
    fn parse_bolt11_nano_non_whole_sats_rejected() {
        // lnbc3n = 3 nBTC = 0.3 sats (not a whole sat) → None
        assert_eq!(parse_bolt11_amount_sats(&fake_invoice("lnbc3n")), None);
    }

    #[test]
    fn parse_bolt11_nano_whole_sats_ok() {
        // lnbc10n = 10 nBTC = 1 sat
        assert_eq!(parse_bolt11_amount_sats(&fake_invoice("lnbc10n")), Some(1));
    }
}
