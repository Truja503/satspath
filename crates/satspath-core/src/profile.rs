use serde::{Deserialize, Serialize};

use crate::ark::ArkOwnershipProof;
use crate::pointer::BitcoinNetwork;

/// A single payment method supported by the profile owner.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum PaymentMethod {
    /// On-chain Bitcoin. Multiple entries are encouraged for privacy.
    Onchain {
        label: String,
        #[serde(default = "default_bitcoin_network")]
        network: BitcoinNetwork,
        address: String,
        #[serde(default)]
        pubkey_hint: Option<String>,
        #[serde(default)]
        descriptor_hint: Option<String>,
    },
    /// Lightning Network via LNURL, Lightning Address, or BOLT12.
    Lightning {
        label: String,
        #[serde(default)]
        lightning_address: Option<String>,
        #[serde(default)]
        lnurl: Option<String>,
        #[serde(default)]
        bolt12: Option<String>,
        #[serde(default)]
        receiver_pubkey: Option<String>,
    },
    /// Ark virtual UTXO protocol.
    Ark {
        label: String,
        server: String,
        pubkey: String,
        #[serde(default)]
        vtxo_pointer: Option<String>,
        #[serde(default)]
        proof: Option<ArkOwnershipProof>,
        #[serde(default)]
        expires_at: Option<i64>,
    },
}

fn default_bitcoin_network() -> BitcoinNetwork {
    BitcoinNetwork::Mainnet
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

    /// A stable, public, privacy-safe identifier for this method.
    ///
    /// An ownership proof is bound to this descriptor so it cannot be lifted
    /// from one method and replayed onto another. It never contains private
    /// material (no xprv, descriptor, seed, etc.) — only public pointers.
    pub fn ownership_descriptor(&self) -> String {
        match self {
            PaymentMethod::Onchain {
                network, address, ..
            } => {
                let net = match network {
                    BitcoinNetwork::Mainnet => "mainnet",
                    BitcoinNetwork::Testnet => "testnet",
                    BitcoinNetwork::Regtest => "regtest",
                };
                format!("onchain:{net}:{address}")
            }
            PaymentMethod::Lightning {
                lightning_address,
                lnurl,
                bolt12,
                label,
                ..
            } => {
                if let Some(addr) = lightning_address {
                    format!("ln-address:{}", addr.trim().to_ascii_lowercase())
                } else if let Some(url) = lnurl {
                    format!("lnurl:{url}")
                } else if let Some(offer) = bolt12 {
                    format!("bolt12:{offer}")
                } else {
                    format!("lightning:{label}")
                }
            }
            PaymentMethod::Ark { pubkey, .. } => format!("ark:{pubkey}"),
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
    /// Ownership-proof attestations, one per (proven) method, bound to the
    /// method's [`PaymentMethod::ownership_descriptor`].
    ///
    /// Omitted from the wire when empty, so profiles authored before ownership
    /// proofs existed serialize — and verify — byte-for-byte identically. The
    /// identity signature commits to this list, making attestations
    /// tamper-evident.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub method_verifications: Vec<crate::ownership::MethodVerification>,
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

/// Non-custodial invite state for an identifier with no published profile yet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InviteRecord {
    pub invite_id: String,
    pub identifier_hash: String,
    pub display_hint: String,
    pub amount_sats: u64,
    pub memo: Option<String>,
    pub sender_fingerprint: String,
    pub status: InviteStatus,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InviteStatus {
    Created,
    EmailSent,
    ClaimedWithPublicProfile,
    Expired,
    Cancelled,
}

/// Public claim policy metadata only. It is never sufficient to spend funds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClaimPolicy {
    SingleSig {
        receiver_pubkey: String,
    },
    Multisig {
        threshold: u8,
        pubkeys: Vec<String>,
        descriptor: Option<String>,
    },
    FutureTaproot {
        internal_key: String,
        script_policy_hint: Option<String>,
    },
}
