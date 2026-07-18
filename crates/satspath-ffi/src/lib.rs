//! satspath-ffi - UniFFI bindings for SatsPath
//!
//! This crate provides the FFI layer that exposes satspath-core and satspath-router
//! to Kotlin (Android), Swift (iOS), and TypeScript (React Native / Tauri / PWA)

// Include the generated UniFFI scaffolding
include!(concat!(env!("OUT_DIR"), "/satspath.uniffi.rs"));

// Our implementation modules
mod resolver;
mod router;
mod profile;
mod identity;
mod p2p;
mod convert;
mod satspath_impl;

// Re-export the generated types so they can be used in other modules
pub use self::{
    r#Identity as Identity,
    r#PaymentMethod as PaymentMethod,
    r#PaymentProfile as PaymentProfile,
    r#SignedPaymentProfile as SignedPaymentProfile,
    r#QuoteRequest as QuoteRequest,
    r#RouteQuote as RouteQuote,
    r#FeeEstimate as FeeEstimate,
    r#QuoteResponse as QuoteResponse,
    r#Invite as Invite,
    r#QuoteRecipient as QuoteRecipient,
    r#InviteRecord as InviteRecord,
    r#InviteStatus as InviteStatus,
    r#FfiError as FfiError,
    r#ExecutionMode as ExecutionMode,
    r#OnchainMethod as OnchainMethod,
    r#LightningMethod as LightningMethod,
    r#ArkMethod as ArkMethod,
    r#ArkOwnershipProof as ArkOwnershipProof,
    r#KeyRotation as KeyRotation,
    r#MethodVerification as MethodVerification,
    r#ProfileResolver as ProfileResolver,
    r#ResolverChain as ResolverChain,
    r#Router as Router,
    r#Satspath as Satspath,
};

mod resolver;
mod router;
mod profile;
mod identity;
mod p2p;
mod convert;
mod satspath_impl;

#[uniffi::export]
fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}