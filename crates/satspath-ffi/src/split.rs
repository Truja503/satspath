//! Split payments FFI

use satspath_router::{
    route_split_payment, validate_split_request, calculate_split_amounts,
    SplitPaymentRequest, SplitRecipient, SplitPaymentRoute, SplitPaymentRoutingResult,
};
use satspath_core::FeeEstimate;
use uniffi::Object;

/// Convert FFI SplitPaymentRequest to core
impl From<SplitPaymentRequest> for satspath_core::SplitPaymentRequest {
    fn from(s: SplitPaymentRequest) -> Self {
        satspath_core::SplitPaymentRequest {
            version: s.version,
            total_amount_sats: s.total_amount_sats,
            splits: s.splits.into_iter().map(|r| r.into()).collect(),
            memo: s.memo,
        }
    }
}

/// Convert core SplitPaymentRequest to FFI
impl From<satspath_core::SplitPaymentRequest> for SplitPaymentRequest {
    fn from(s: satspath_core::SplitPaymentRequest) -> Self {
        SplitPaymentRequest {
            version: s.version,
            total_amount_sats: s.total_amount_sats,
            splits: s.splits.into_iter().map(|r| r.into()).collect(),
            memo: s.memo,
        }
    }
}

/// Convert FFI SplitRecipient to core
impl From<SplitRecipient> for satspath_core::SplitRecipient {
    fn from(r: SplitRecipient) -> Self {
        satspath_core::SplitRecipient {
            alias: r.alias,
            percent: r.percent,
        }
    }
}

/// Convert core SplitRecipient to FFI
impl From<satspath_core::SplitRecipient> for SplitRecipient {
    fn from(r: satspath_core::SplitRecipient) -> Self {
        SplitRecipient {
            alias: r.alias,
            percent: r.percent,
        }
    }
}

/// Convert core SplitPaymentRoute to FFI
impl From<satspath_router::SplitPaymentRoute> for SplitPaymentRoute {
    fn from(r: satspath_router::SplitPaymentRoute) -> Self {
        SplitPaymentRoute {
            recipient_alias: r.recipient_alias,
            percent: r.percent,
            amount_sats: r.amount_sats,
            route: r.route.into(),
        }
    }
}

/// Convert core SplitPaymentRoutingResult to FFI
impl From<satspath_router::SplitPaymentRoutingResult> for SplitPaymentRoutingResult {
    fn from(r: satspath_router::SplitPaymentRoutingResult) -> Self {
        SplitPaymentRoutingResult {
            routes: r.routes.into_iter().map(|r| r.into()).collect(),
            total_fee_sats: r.total_fee_sats,
            all_routed: r.all_routed,
            errors: r.errors,
        }
    }
}

/// Route a split payment
#[uniffi::export(async_runtime = "tokio")]
pub async fn route_split_payment_ffi(request: SplitPaymentRequest, fees: FeeEstimate) -> Result<SplitPaymentRoutingResult, satspath_router::SatsPathError> {
    let req: satspath_core::SplitPaymentRequest = request.into();
    let fee_est: satspath_router::FeeEstimate = fees.into();
    let result = route_split_payment(&req, &fee_est)?;
    Ok(result.into())
}

/// Validate a split payment request
#[uniffi::export]
pub fn validate_split_request_ffi(request: SplitPaymentRequest) -> Result<(), satspath_router::SatsPathError> {
    let req: satspath_core::SplitPaymentRequest = request.into();
    validate_split_request(&req).map_err(Into::into)
}

/// Calculate split amounts
#[uniffi::export]
pub fn calculate_split_amounts_ffi(request: SplitPaymentRequest) -> Vec<(String, u64)> {
    let req: satspath_core::SplitPaymentRequest = request.into();
    calculate_split_amounts(&req)
}