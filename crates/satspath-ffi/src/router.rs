//! Router FFI — exposes payment routing to foreign platforms.

use crate::types::*;

/// Select the best route for a payment given a fee estimate.
#[uniffi::export(async_runtime = "tokio")]
pub async fn select_route(
    request: FfiQuoteRequest,
    fees: FfiFeeEstimate,
) -> Result<FfiRouteQuote, FfiError> {
    let req: satspath_router::RouteRequest = request.into();
    let fee_est: satspath_router::FeeEstimate = fees.into();
    let quote = satspath_router::select_route_with_fees(&req, &fee_est)
        .map_err(|e| FfiError::Other { reason: e.to_string() })?;
    Ok(quote.into())
}

/// Fetch current fee estimates from mempool.space.
#[uniffi::export(async_runtime = "tokio")]
pub async fn fetch_fee_estimate() -> Result<FfiFeeEstimate, FfiError> {
    satspath_router::fees::fetch_fee_estimate()
        .await
        .map(Into::into)
        .map_err(|e| FfiError::NetworkError { reason: e.to_string() })
}

/// Build a QR-scannable payment payload for a method.
#[uniffi::export]
pub fn build_qr_payload(
    method: FfiPaymentMethod,
    amount_sats: u64,
) -> Result<String, FfiError> {
    satspath_router::build_qr_payload(&method.into(), amount_sats)
        .map_err(|e| FfiError::Other { reason: e.to_string() })
}