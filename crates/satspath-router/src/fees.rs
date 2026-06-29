use serde::Deserialize;

/// Recommended fee rates from mempool.space.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeEstimate {
    pub fastest_fee: u64,
    pub half_hour_fee: u64,
    pub hour_fee: u64,
    pub economy_fee: u64,
    pub minimum_fee: u64,
}

impl FeeEstimate {
    /// Safe fallback when the API is unavailable.
    pub fn fallback() -> Self {
        FeeEstimate {
            fastest_fee: 10,
            half_hour_fee: 7,
            hour_fee: 5,
            economy_fee: 3,
            minimum_fee: 1,
        }
    }
}

/// Fetch current fee estimates from mempool.space. Falls back to safe mock on error.
pub async fn fetch_fee_estimate() -> FeeEstimate {
    match try_fetch_fee_estimate().await {
        Ok(est) => est,
        Err(_) => FeeEstimate::fallback(),
    }
}

async fn try_fetch_fee_estimate() -> anyhow::Result<FeeEstimate> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    let est = client
        .get("https://mempool.space/api/v1/fees/recommended")
        .send()
        .await?
        .json::<FeeEstimate>()
        .await?;
    Ok(est)
}
