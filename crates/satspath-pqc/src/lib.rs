//! # satspath-pqc — Post-Quantum Cryptography for SatsPath
//!
//! This crate provides hybrid (classical + post-quantum) cryptographic
//! primitives to future-proof SatsPath against quantum computing threats.
//!
//! ## Design Philosophy
//!
//! **Hybrid-first**: All operations combine a classical algorithm with a
//! post-quantum algorithm. If either is broken, the other still provides
//! security. This matches NIST's recommended migration strategy.
//!
//! ## Standards & Algorithms
//!
//! - **Digital Signatures**: ML-DSA-65 (NIST FIPS 204 / Dilithium) combined with Schnorr (BIP-340 / secp256k1)
//! - **Key Encapsulation**: ML-KEM-768 (NIST FIPS 203 / Kyber) combined with X25519 (RFC 7748)
//!
//! ## Modules
//!
//! - [`hybrid_sig`] — Hybrid signatures (Schnorr + ML-DSA-65)
//! - [`hybrid_kem`] — Hybrid key encapsulation (X25519 + ML-KEM-768)
//! - [`types`] — Shared types for PQC-enabled profiles

pub mod hybrid_kem;
pub mod hybrid_sig;
pub mod types;
