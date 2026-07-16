use serde::{Deserialize, Serialize};
use std::env;

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

/// Fallback fees when network is unavailable (conservative estimates)
pub const FALLBACK_FEES: FeeEstimate = FeeEstimate {
    fastest_fee: 20,
    half_hour_fee: 15,
    hour_fee: 10,
    economy_fee: 5,
    minimum_fee: 1,
};

#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'a str,
    id: &'a str,
    method: &'a str,
    params: Vec<u64>,
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: Option<SmartFeeResult>,
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SmartFeeResult {
    feerate: Option<f64>,
    #[allow(dead_code)]
    errors: Option<Vec<String>>,
}

#[cfg(feature = "std")]
mod native_fees {
    use super::*;
    use reqwest::Client;

    async fn fetch_target_fee(
        target: u64,
        client: &Client,
        url: &str,
        auth: &Option<(String, Option<String>)>,
    ) -> Option<u64> {
        let req = RpcRequest {
            jsonrpc: "1.0",
            id: "satspath",
            method: "estimatesmartfee",
            params: vec![target],
        };
        let mut builder = client.post(url).json(&req);
        if let Some((user, pass)) = auth {
            builder = builder.basic_auth(user.clone(), pass.clone());
        }
        let res = builder.send().await.ok()?.json::<RpcResponse>().await.ok()?;
        if res.error.is_some() {
            return None;
        }
        let feerate_btc_kvb = res.result?.feerate?;
        let sat_vb = (feerate_btc_kvb * 100_000.0).ceil() as u64;
        Some(std::cmp::max(1, sat_vb))
    }

    pub async fn try_bitcoin_core_fee() -> Result<FeeEstimate> {
        let url = env::var("BITCOIN_RPC_URL").ok();
        if url.is_none() {
            return Err(SatsPathError::NetworkError(
                "BITCOIN_RPC_URL not set".into(),
            ));
        }
        let url = url.unwrap();
        let user = env::var("BITCOIN_RPC_USER").ok();
        let pass = env::var("BITCOIN_RPC_PASS").ok();
        let auth = user.map(|u| (u, pass));

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .map_err(|e| SatsPathError::NetworkError(e.to_string()))?;

        let fastest = fetch_target_fee(1, &client, &url, &auth).await;
        let half_hour = fetch_target_fee(3, &client, &url, &auth).await;
        let hour = fetch_target_fee(6, &client, &url, &auth).await;

        if fastest.is_none() || half_hour.is_none() || hour.is_none() {
            return Err(SatsPathError::NetworkError(
                "Failed to fetch smart fees from Bitcoin Core".into(),
            ));
        }

        Ok(FeeEstimate {
            fastest_fee: fastest.unwrap(),
            half_hour_fee: half_hour.unwrap(),
            hour_fee: hour.unwrap(),
            economy_fee: std::cmp::max(1, hour.unwrap() / 2),
            minimum_fee: 1,
        })
    }

    pub async fn try_fetch_fee_estimate() -> Result<FeeEstimate> {
        let client = Client::builder()
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
}

#[cfg(feature = "wasm")]
mod wasm_fees {
    use super::*;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode, Response, window};

    #[derive(Deserialize)]
    struct DohResponse {
        #[serde(rename = "Answer")]
        answer: Option<Vec<DohAnswer>>,
    }

    #[derive(Deserialize)]
    struct DohAnswer {
        data: String,
    }

    pub async fn fetch_fee_estimate() -> Result<FeeEstimate> {
        let opts = RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(RequestMode::Cors);

        let request = Request::new_with_str_and_init(
            "https://mempool.space/api/v1/fees/recommended",
            &opts,
        )?;

        let window = window().ok_or_else(|| SatsPathError::NetworkError("no window".into()))?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
        let response: Response = resp_value.dyn_into()?;

        if !response.ok() {
            return Ok(FALLBACK_FEES);
        }

        let json = JsFuture::from(response.json()?).await?;
        let estimate: MempoolFeeEstimate = serde_wasm_bindgen::from_value(json)
            .map_err(|e| SatsPathError::SerializationError(e.to_string()))?;

        Ok(estimate.into())
    }
}

/// Fetch current fee estimates.
/// - Native: tries Bitcoin Core RPC first, falls back to mempool.space
/// - WASM: uses mempool.space directly via browser fetch
pub async fn fetch_fee_estimate() -> Result<FeeEstimate> {
    #[cfg(feature = "std")]
    {
        if let Ok(fee) = native_fees::try_bitcoin_core_fee().await {
            return Ok(fee);
        }
        match native_fees::try_fetch_fee_estimate().await {
            Ok(fee) => Ok(fee),
            Err(_) => Ok(FALLBACK_FEES),
        }
    }
    #[cfg(feature = "wasm")]
    {
        match wasm_fees::fetch_fee_estimate().await {
            Ok(fee) => Ok(fee),
            Err(_) => Ok(FALLBACK_FEES),
        }
    }
    #[cfg(not(any(feature = "std", feature = "wasm")))]
    {
        Ok(FALLBACK_FEES)
    }
}

/// Synchronous getter for fallback fees (useful for tests/WASM)
pub fn fallback_fees() -> FeeEstimate {
    FALLBACK_FEES
}