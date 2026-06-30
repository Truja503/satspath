use serde::Deserialize;

use satspath_core::{Result, SatsPathError};

/// Recommended fee rates from mempool.space.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MempoolFeeEstimate {
    pub fastest_fee: u64,
    pub half_hour_fee: u64,
    pub hour_fee: u64,
    pub economy_fee: u64,
    pub minimum_fee: u64,
}

/// Internal fee estimate type used by the router.
#[derive(Debug, Clone, Default)]
pub struct FeeEstimate {
    pub fastest_fee: u64,
    pub half_hour_fee: u64,
    pub hour_fee: u64,
    pub economy_fee: u64,
    pub minimum_fee: u64,
}

impl From<MempoolFeeEstimate> for FeeEstimate {
    fn from(e: MempoolFeeEstimate) -> Self {
        FeeEstimate {
            fastest_fee: e.fastest_fee,
            half_hour_fee: e.half_hour_fee,
            hour_fee: e.hour_fee,
            economy_fee: e.economy_fee,
            minimum_fee: e.minimum_fee,
        }
    }
}

/// Fetch current fee estimates from mempool.space.
/// Returns error if API is unavailable (fail-closed).
pub async fn fetch_fee_estimate() -> Result<FeeEstimate> {
    try_fetch_fee_estimate().await
}

async fn try_fetch_fee_estimate() -> Result<FeeEstimate> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| SatsPathError::NetworkError(e.to_string()))?;
    let est = client
        .get("https://mempool.space/api/v1/fees/recommended")
        .send()
        .await
        .map_err(|e| SatsPathError::NetworkError(e.to_string()))?
        .json::<MempoolFeeEstimate>()
        .await
        .map_err(|e| SatsPathError::NetworkError(e.to_string()))?;
    Ok(est.into())
}
