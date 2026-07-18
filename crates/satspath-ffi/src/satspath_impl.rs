//! Satspath implementation - implements the generated Satspath trait

use satspath_core::{
    crypto::verify_signed_profile,
    profile::{PaymentMethod as CorePaymentMethod, PaymentProfile as CorePaymentProfile, SignedPaymentProfile as CoreSignedPaymentProfile},
    SatsPathError,
};
use satspath_router::{
    select_route, select_route_with_fees, FeeEstimate as RouterFeeEstimate, RouteRequest, build_qr_payload,
    fees::fetch_fee_estimate, quote, quote_with_resolver,
};
use crate::resolver::ResolverChain;
use crate::identity::{create_identity, fingerprint_pubkey};
use crate::p2p::{start_p2p_bridge_ffi, stop_p2p_bridge_ffi};
use crate::router::{quote_request_to_route_request, route_quote_to_ffi};
use crate::convert::*;
use crate::identity::{verify_rotation_ffi as verify_rotation_impl};

use satspath_core::crypto::verify_signed_profile as verify_signed_profile_core;
use satspath_router::fees::fetch_fee_estimate as fetch_fee_estimate_router;
use satspath_router::{quote_with_resolver as quote_with_resolver_router, quote as quote_router};

use secp256k1::SecretKey;
use std::str::FromStr;

use satspath_core::{SignedPaymentProfile as CoreSignedPaymentProfile, SatsPathError};
use satspath_router::{FeeEstimate as RouterFeeEstimate, RouteRequest, build_qr_payload as build_qr_payload_router};
use satspath_router::urgency::PaymentUrgency;

// Implement the generated Satspath trait for a simple unit type
pub struct SatspathImpl;

impl crate::Satspath for SatspathImpl {
    async fn createIdentity(&self) -> crate::Identity {
        crate::identity::create_identity().into()
    }

    async fn loadProfile(&self, _alias: String) -> Option<crate::SignedPaymentProfile> {
        // Platform-specific storage not implemented in core
        None
    }

    async fn quote(&self, recipient: String, amount_sats: u64) -> crate::QuoteResponse {
        let resolver = crate::resolver::ResolverChain::new();
        satspath_router::quote_with_resolver(&resolver.inner, &recipient, amount_sats).await
    }

    async fn resolve(&self, alias: String) -> crate::SignedPaymentProfile {
        let resolver = crate::resolver::ResolverChain::new();
        resolver.inner.resolve_alias(&alias).await
            .map(|p| p.into())
            .unwrap_or_else(|e| panic!("Resolve error: {}", e))
    }

    fn verifyProfile(&self, profile: crate::SignedPaymentProfile) -> bool {
        satspath_core::verify_signed_profile(&profile.into()).unwrap_or(false)
    }

    async fn route(&self, request: crate::QuoteRequest) -> crate::RouteQuote {
        let req = crate::router::quote_request_to_route_request(request);
        let fees = satspath_router::fees::fetch_fee_estimate().await.unwrap_or_else(|e| panic!("Fee estimate error: {}", e));
        let route_quote = satspath_router::select_route_with_fees(&req, &fees.into())
            .unwrap_or_else(|e| panic!("Route error: {}", e));
        crate::router::route_quote_to_ffi(route_quote)
    }

    async fn saveProfile(&self, _profile: crate::SignedPaymentProfile) {
        // Platform-specific implementation needed
    }

    async fn startP2pBridge(&self, profile_path: String) {
        crate::p2p::start_p2p_bridge_ffi(profile_path).await
            .unwrap_or_else(|e| panic!("P2P bridge error: {}", e))
    }

    fn stopP2pBridge(&self) {
        crate::p2p::stop_p2p_bridge_ffi();
    }

    async fn rotateKey(&self, alias: String, new_pubkey_hex: String) -> crate::SignedPaymentProfile {
        panic!("rotateKey not fully implemented - requires platform-specific profile storage");
    }

    fn verifyRotation(&self, old_profile: crate::SignedPaymentProfile, new_profile: crate::SignedPaymentProfile) -> bool {
        crate::identity::verify_rotation_ffi(old_profile, new_profile)
    }

    async fn startP2pBridge(&self, profile_path: String) {
        crate::p2p::start_p2p_bridge_ffi(profile_path).await
            .unwrap_or_else(|e| panic!("P2P bridge error: {}", e))
    }

    fn stopP2pBridge(&self) {
        crate::p2p::stop_p2p_bridge_ffi();
    }
}

// Implement the generated Router trait
pub struct RouterImpl;

impl crate::Router for RouterImpl {
    async fn selectRoute(&self, request: crate::QuoteRequest, fees: crate::FeeEstimate) -> crate::RouteQuote {
        let req = crate::router::quote_request_to_route_request(request);
        let route_quote = satspath_router::select_route_with_fees(&req, &fees.into())
            .unwrap_or_else(|e| panic!("Route error: {}", e));
        crate::router::route_quote_to_ffi(route_quote)
    }

    async fn fetchFeeEstimate(&self) -> crate::FeeEstimate {
        satspath_router::fees::fetch_fee_estimate().await.unwrap_or_else(|e| panic!("Fee estimate error: {}", e)).into()
    }

    fn buildQrPayload(&self, method: crate::PaymentMethod, amount_sats: u64) -> String {
        satspath_router::build_qr_payload(&method.into(), amount_sats).unwrap_or_else(|e| panic!("QR payload error: {}", e))
    }
}

// Implement the generated ProfileResolver trait
pub struct ProfileResolverImpl;

impl crate::ProfileResolver for ProfileResolverImpl {
    async fn resolveAlias(&self, alias: String) -> crate::SignedPaymentProfile {
        let resolver = crate::resolver::ResolverChain::new();
        resolver.inner.resolve_alias(&alias).await
            .map(|p| p.into())
            .unwrap_or_else(|e| panic!("Resolve error: {}", e))
    }
}

// Implement the generated ResolverChain trait
pub struct ResolverChainImpl {
    inner: crate::resolver::ResolverChain,
}

impl crate::ResolverChain for ResolverChainImpl {
    async fn resolveAlias(&self, alias: String) -> crate::SignedPaymentProfile {
        self.inner.resolve_alias(alias).await
            .unwrap_or_else(|e| panic!("Resolve error: {}", e))
    }
}