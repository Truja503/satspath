//! satspath-ffi — UniFFI bindings for SatsPath
//!
//! This crate provides the FFI layer that exposes satspath-core and satspath-router
//! to Kotlin (Android), Swift (iOS), and TypeScript (React Native / Tauri / PWA).
//!
//! We use proc-macro based UniFFI (uniffi::setup_scaffolding + #[derive(uniffi::Record)],
//! #[uniffi::export], etc.) rather than UDL-file scaffolding. This avoids the
//! dual-definition conflict and gives us full Rust type safety.

uniffi::setup_scaffolding!();

// ── FFI type definitions ──────────────────────────────────────────────────────
pub mod types;

// ── Conversion between internal core types and FFI types ──────────────────────
pub mod convert;

// ── FFI exported functions ────────────────────────────────────────────────────
pub mod resolver;
pub mod router;
pub mod identity;
pub mod crypto;
pub mod profile;
pub mod p2p;

#[uniffi::export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}