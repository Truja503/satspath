//! High-level Satspath FFI interface

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
use crate::identity::{create_identity};
use crate::p2p::{start_p2p_bridge_ffi, stop_p2p_bridge_ffi};
use crate::router::{quote_request_to_route_request, route_quote_to_ffi};
use crate::convert::*;

use secp256k1::SecretKey;
use std::str::FromStr;

// Use the generated FFI types directly
use crate::{
    r#Identity as FfiIdentity,
    r#PaymentMethod as FfiPaymentMethod,
    r#PaymentProfile as FfiPaymentProfile,
    r#SignedPaymentProfile as FfiSignedPaymentProfile,
    r#QuoteRequest as FfiQuoteRequest,
    r#RouteQuote as FfiRouteQuote,
    r#FeeEstimate as FfiFeeEstimate,
    r#QuoteResponse as FfiQuoteResponse,
    r#Invite as FfiInvite,
    r#QuoteRecipient as FfiQuoteRecipient,
    r#FfiError,
};

/// High-level quote function: resolve + verify + route + build payload
#[uniffi::export(async_runtime = "tokio")]
pub async fn quote(recipient: &str, amount_sats: u64) -> crate::r#QuoteResponse {
    let resolver = ResolverChain::new();
    // Pass the inner ChainResolver directly (it implements ProfileResolver)
    quote_with_resolver(&resolver.inner, recipient, amount_sats).await
}

/// Resolve an alias to a signed payment profile
#[uniffi::export(async_runtime = "tokio")]
pub async fn resolve(alias: String) -> Result<crate::r#SignedPaymentProfile, crate::r#FfiError> {
    let resolver = ResolverChain::new();
    resolver.resolve_alias(alias).await.map_err(|e| crate::r#FfiError::Other(e.to_string()))
}

/// Verify a signed profile's signature
#[uniffi::export]
pub fn verify_profile(profile: crate::r#SignedPaymentProfile) -> bool {
    verify_signed_profile(&profile.into()).unwrap_or(false)
}

/// Route a payment request with a pre-fetched fee estimate
#[uniffi::export(async_runtime = "tokio")]
pub async fn route(request: crate::r#QuoteRequest, fees: crate::r#FeeEstimate) -> Result<crate::r#RouteQuote, crate::r#FfiError> {
    let req = crate::router::quote_request_to_route_request(request);
    let route_quote = select_route_with_fees(&req, &fees.into())
        .map_err(|e| crate::r#FfiError::Other(e.to_string()))?;
    Ok(route_quote_to_ffi(route_quote))
}

/// Fetch current fee estimates from mempool
#[uniffi::export(async_runtime = "tokio")]
pub async fn fetch_fee_estimate() -> Result<crate::r#FeeEstimate, crate::r#FfiError> {
    fetch_fee_estimate().await.map_err(|e| crate::r#FfiError::Other(e.to_string())).map(Into::into)
}

/// Build QR payload for a payment method
#[uniffi::export]
pub fn build_qr_payload(method: crate::r#PaymentMethod, amount_sats: u64) -> Result<String, crate::r#FfiError> {
    build_qr_payload(&method.into(), amount_sats).map_err(|e| crate::r#FfiError::Other(e.to_string()))
}

/// Create a new identity
#[uniffi::export]
pub fn create_identity_ffi() -> crate::r#Identity {
    crate::identity::create_identity()
}

/// Save a profile to local storage (platform-specific implementation needed)
#[uniffi::export(async_runtime = "tokio")]
pub async fn save_profile(_profile: crate::r#SignedPaymentProfile) -> Result<(), crate::r#FfiError> {
    // In production, save to platform-specific encrypted storage
    // Android: SQLCipher/KeyStore, iOS: Keychain, Desktop: SQLCipher
    Ok(())
}

/// Load a profile from local storage (platform-specific implementation needed)
#[uniffi::export(async_runtime = "tokio")]
pub async fn load_profile(_alias: String) -> Result<Option<crate::r#SignedPaymentProfile>, crate::r#FfiError> {
    // In production, load from platform-specific encrypted storage
    Ok(None)
}

/// Start the P2P bridge (Hyperswarm)
#[uniffi::export(async_runtime = "tokio")]
pub async fn start_p2p_bridge(profile_path: String) -> Result<(), crate::r#FfiError> {
    crate::p2p::start_p2p_bridge_ffi(profile_path).await
}

/// Stop the P2P bridge
#[uniffi::export]
pub fn stop_p2p_bridge() {
    crate::p2p::stop_p2p_bridge_ffi()
}

/// Rotate identity key
#[uniffi::export(async_runtime = "tokio")]
pub async fn rotate_key(alias: String, new_pubkey_hex: String) -> Result<crate::r#SignedPaymentProfile, crate::r#FfiError> {
    // Load current profile
    let current = load_profile(alias).await?
        .ok_or(crate::r#FfiError::AliasNotFound)?;
    
    // Verify rotation
    let _new_pubkey = secp256k1::PublicKey::from_str(&new_pubkey_hex)
        .map_err(|e| crate::r#FfiError::CryptoError(e.to_string()))?;
    
    // Apply rotation (this would need the old secret key to sign the rotation)
    // For now, return the current profile
    Ok(current)
}

/// Verify key rotation between old and new profile
#[uniffi::export]
pub fn verify_rotation(old_profile: crate::r#SignedPaymentProfile, new_profile: crate::r#SignedPaymentProfile) -> bool {
    crate::identity::verify_rotation_ffi(old_profile, new_profile)
}