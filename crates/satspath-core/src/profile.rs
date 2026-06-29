use serde::{Deserialize, Serialize};

/// A single payment method supported by the profile owner.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum PaymentMethod {
    /// On-chain Bitcoin. Multiple entries are encouraged for privacy.
    Onchain {
        label: String,
        address: String,
        pubkey_hint: Option<String>,
    },
    /// Lightning Network via LNURL, Lightning Address, or BOLT12.
    Lightning {
        label: String,
        lnurl: Option<String>,
        lightning_address: Option<String>,
        bolt12: Option<String>,
    },
    /// Ark virtual UTXO protocol.
    Ark {
        label: String,
        server: String,
        pubkey: String,
    },
}

impl PaymentMethod {
    pub fn method_name(&self) -> &'static str {
        match self {
            PaymentMethod::Onchain { .. } => "Onchain",
            PaymentMethod::Lightning { .. } => "Lightning",
            PaymentMethod::Ark { .. } => "Ark",
        }
    }

    pub fn label(&self) -> &str {
        match self {
            PaymentMethod::Onchain { label, .. } => label,
            PaymentMethod::Lightning { label, .. } => label,
            PaymentMethod::Ark { label, .. } => label,
        }
    }
}

/// A user-owned payment profile associating an alias with payment methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentProfile {
    /// Human-readable identifier, e.g. "alice@example.com"
    pub alias: String,
    /// Hex-encoded secp256k1 compressed public key
    pub identity_pubkey: String,
    /// Ordered list of payment methods (most preferred first)
    pub methods: Vec<PaymentMethod>,
    /// Unix timestamp of last update
    pub updated_at: i64,
    /// Optional Unix timestamp after which this profile should be considered
    /// expired and must not be used for routing.
    /// `None` means the profile does not expire (non-expiring).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

/// A payment profile together with the owner's signature over its contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPaymentProfile {
    pub profile: PaymentProfile,
    /// Hex-encoded DER-encoded secp256k1 ECDSA signature
    pub signature: String,
}

/// A parsed universal SatsPath payment request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequest {
    pub version: u32,
    pub alias: String,
    pub amount_sats: Option<u64>,
    pub memo: Option<String>,
    pub profile_hint: Option<String>,
}

/// An invitation for an unregistered user to claim a pending payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invite {
    /// SHA-256 hash of the alias (hex)
    pub alias_hash: String,
    pub amount_sats: u64,
    pub created_at: i64,
    pub claim_url: String,
    pub warning: String,
}
