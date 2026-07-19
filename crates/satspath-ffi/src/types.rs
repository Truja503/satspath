//! FFI type definitions — single source of truth for all UniFFI-exported types.
//!
//! Every type here is annotated with `#[derive(uniffi::Record)]` or `#[uniffi::Enum]`
//! so UniFFI picks them up via proc-macros. No UDL file needed.

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiError {
    #[error("Alias not found: {reason}")]
    AliasNotFound { reason: String },
    #[error("Crypto error: {reason}")]
    CryptoError { reason: String },
    #[error("Network error: {reason}")]
    NetworkError { reason: String },
    #[error("Invalid input: {reason}")]
    InvalidInput { reason: String },
    #[error("{reason}")]
    Other { reason: String },
}

impl From<satspath_core::SatsPathError> for FfiError {
    fn from(e: satspath_core::SatsPathError) -> Self {
        match e {
            satspath_core::SatsPathError::AliasNotFound(s) => FfiError::AliasNotFound { reason: s },
            satspath_core::SatsPathError::InvalidSignature => {
                FfiError::CryptoError { reason: "invalid signature".into() }
            }
            satspath_core::SatsPathError::CryptoError(s) => FfiError::CryptoError { reason: s },
            satspath_core::SatsPathError::NetworkError(s) => FfiError::NetworkError { reason: s },
            other => FfiError::Other { reason: other.to_string() },
        }
    }
}

impl From<anyhow::Error> for FfiError {
    fn from(e: anyhow::Error) -> Self {
        FfiError::Other { reason: e.to_string() }
    }
}

// ── Identity ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiIdentity {
    pub pubkey_hex: String,
    pub secret_key_hex: String,
    pub fingerprint: String,
}

// ── Payment Methods ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiOnchainMethod {
    pub label: String,
    pub network: String,
    pub address: Option<String>,
    pub silent_payment_pubkey: Option<String>,
    pub pubkey_hint: Option<String>,
    pub descriptor_hint: Option<String>,
    pub address_list: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiLightningMethod {
    pub label: String,
    pub lightning_address: Option<String>,
    pub lnurl: Option<String>,
    pub bolt12: Option<String>,
    pub receiver_pubkey: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiArkMethod {
    pub label: String,
    pub server: String,
    pub pubkey: String,
    pub vtxo_pointer: Option<String>,
    pub opaque_uri: Option<String>,
    pub proof: Option<FfiArkOwnershipProof>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiPaymentMethod {
    Onchain { method: FfiOnchainMethod },
    Lightning { method: FfiLightningMethod },
    Ark { method: FfiArkMethod },
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiArkOwnershipProof {
    pub message: String,
    pub signature: String,
    pub pubkey: String,
}

// ── Key Rotation ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiKeyRotation {
    pub previous_pubkey: String,
    pub new_pubkey: String,
    pub authorization_signature: String,
    pub rotated_at: i64,
}

// ── Method Verification ───────────────────────────────────────────────────────

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiMethodVerification {
    pub method_descriptor: String,
    pub proof_type: String,
    pub proof_data: String,
    pub verified_at: i64,
}

// ── Profile ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiPaymentProfile {
    pub alias: String,
    pub identity_pubkey: String,
    pub methods: Vec<FfiPaymentMethod>,
    pub updated_at: i64,
    pub expires_at: Option<i64>,
    pub sequence: Option<u64>,
    pub preferences: Vec<String>,
    pub nonce: Option<String>,
    pub rotation: Option<FfiKeyRotation>,
    pub method_verifications: Vec<FfiMethodVerification>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiSignedPaymentProfile {
    pub profile: FfiPaymentProfile,
    pub signature: String,
}

// ── Fee Estimate ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiFeeEstimate {
    pub fastest_fee: u64,
    pub half_hour_fee: u64,
    pub hour_fee: u64,
    pub economy_fee: u64,
    pub minimum_fee: u64,
}

// ── Execution Mode ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiExecutionMode {
    Preview,
    MainnetPreview,
    TestnetExperimental,
    ManualWallet,
}

// ── Route Quote ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiRouteQuote {
    pub selected_method: FfiPaymentMethod,
    pub estimated_fee_sats: u64,
    pub estimated_confirmation: String,
    pub reason: String,
    pub execution: FfiExecutionMode,
    pub wallet_hint: String,
}

// ── Quote Request ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiQuoteRequest {
    pub recipient: String,
    pub amount_sats: u64,
    pub signed_profile: FfiSignedPaymentProfile,
    pub urgency: String,
    pub max_fee_sats: Option<u64>,
    pub max_fee_percent: Option<f64>,
}

// ── Quote Response ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiQuoteRecipient {
    pub alias: String,
    pub verified: bool,
    pub profile_signature_verified: bool,
    pub identifier_verified: bool,
    pub identifier_verification: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiInvite {
    pub alias_hash: String,
    pub amount_sats: u64,
    pub created_at: i64,
    pub expires_at: i64,
    pub claim_url: String,
    pub warning: String,
    pub sender_signature: Option<String>,
    pub sender_pubkey: Option<String>,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiInviteStatus {
    Created,
    EmailSent,
    ClaimedWithPublicProfile,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiInviteRecord {
    pub invite_id: String,
    pub identifier_hash: String,
    pub display_hint: String,
    pub amount_sats: u64,
    pub memo: Option<String>,
    pub sender_fingerprint: String,
    pub status: FfiInviteStatus,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiQuoteResponse {
    Ok {
        recipient: FfiQuoteRecipient,
        selected_method: FfiPaymentMethod,
        fee_sats: u64,
        eta: String,
        reason: String,
        qr: String,
        execution: FfiExecutionMode,
        wallet_hint: String,
    },
    NotRegistered {
        invite: FfiInvite,
    },
    NoRoute {
        reason: String,
    },
    InvalidSignature {
        recipient: FfiQuoteRecipient,
    },
}

// ── Split Payments ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiSplitRecipient {
    pub alias: String,
    pub percent: u8,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiSplitPaymentRequest {
    pub version: u32,
    pub total_amount_sats: u64,
    pub splits: Vec<FfiSplitRecipient>,
    pub memo: Option<String>,
}
