//! Router FFI implementation

use satspath_router::{
    select_route_with_fees, FeeEstimate, RouteRequest, build_qr_payload, fees::fetch_fee_estimate,
    urgency::PaymentUrgency,
};
use satspath_core::{SignedPaymentProfile as CoreSignedPaymentProfile, SatsPathError, ExecutionMode};
use uniffi::deps::anyhow::Result;

// Use the generated FFI types from crate root
use crate::{
    RouteQuote,
    FeeEstimate,
    QuoteRequest,
    PaymentMethod,
    FfiError,
};

/// Convert FFI QuoteRequest to internal RouteRequest
pub fn quote_request_to_route_request(request: crate::QuoteRequest) -> satspath_router::RouteRequest {
    let urgency = match request.urgency.as_str() {
        "urgent" => PaymentUrgency::Urgent,
        "commercial" => PaymentUrgency::Commercial,
        "economy" => PaymentUrgency::Economy,
        _ => PaymentUrgency::Normal,
    };

    satspath_router::RouteRequest {
        alias: request.recipient,
        amount_sats: request.amount_sats,
        signed_profile: request.signed_profile.into(),
        urgency,
        max_fee_sats: request.max_fee_sats,
        max_fee_percent: request.max_fee_percent,
    }
}

/// Convert internal RouteQuote to FFI RouteQuote
pub fn route_quote_to_ffi(quote: satspath_router::RouteQuote) -> crate::RouteQuote {
    crate::RouteQuote {
        selected_method: quote.selected_method.into(),
        estimated_fee_sats: quote.estimated_fee_sats.unwrap_or(0),
        estimated_confirmation: quote.estimated_confirmation.unwrap_or_default(),
        reason: quote.reason,
        execution: match quote.execution {
            Some(satspath_core::ExecutionMode::Preview) => crate::ExecutionMode::Preview,
            Some(satspath_core::ExecutionMode::MainnetPreview) => crate::ExecutionMode::MainnetPreview,
            Some(satspath_core::ExecutionMode::TestnetExperimental) => crate::ExecutionMode::TestnetExperimental,
            Some(satspath_core::ExecutionMode::ManualWallet) => crate::ExecutionMode::ManualWallet,
            None => crate::ExecutionMode::Preview,
        },
        wallet_hint: quote.wallet_hint.unwrap_or_default(),
    }
}

/// Implement the generated Router trait
pub struct RouterImpl;

impl crate::Router for RouterImpl {
    async fn selectRoute(&self, request: crate::QuoteRequest, fees: crate::FeeEstimate) -> crate::RouteQuote {
        let req = quote_request_to_route_request(request);
        let quote = satspath_router::select_route_with_fees(&req, &fees.into())
            .unwrap_or_else(|e| panic!("Route error: {}", e));
        route_quote_to_ffi(quote)
    }

    async fn fetchFeeEstimate(&self) -> crate::FeeEstimate {
        satspath_router::fees::fetch_fee_estimate().await
            .unwrap_or_else(|e| panic!("Fee estimate error: {}", e))
            .into()
    }

    fn buildQrPayload(&self, method: crate::PaymentMethod, amount_sats: u64) -> String {
        satspath_router::build_qr_payload(&method.into(), amount_sats).unwrap_or_else(|e| panic!("QR payload error: {}", e))
    }
}