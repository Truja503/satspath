//! Bidirectional conversions between satspath-core types and FFI types.

use crate::types::*;
use satspath_core::{
    PaymentMethod as CorePaymentMethod,
    PaymentProfile as CorePaymentProfile,
    SignedPaymentProfile as CoreSignedPaymentProfile,
    Invite as CoreInvite,
    InviteRecord as CoreInviteRecord,
    InviteStatus as CoreInviteStatus,
    ExecutionMode as CoreExecutionMode,
    KeyRotation as CoreKeyRotation,
    pointer::BitcoinNetwork,
};
use satspath_core::ark::ArkOwnershipProof as CoreArkOwnershipProof;
use satspath_core::ownership::MethodVerification as CoreMethodVerification;
use satspath_router::{
    FeeEstimate as RouterFeeEstimate,
    RouteQuote as CoreRouteQuote,
    urgency::PaymentUrgency,
};

// ── Core → FFI ────────────────────────────────────────────────────────────────

impl From<CorePaymentMethod> for FfiPaymentMethod {
    fn from(m: CorePaymentMethod) -> Self {
        match m {
            CorePaymentMethod::Onchain {
                label, network, address, silent_payment_pubkey,
                pubkey_hint, descriptor_hint, address_list,
            } => FfiPaymentMethod::Onchain {
                method: FfiOnchainMethod {
                    label,
                    network: format!("{:?}", network).to_lowercase(),
                    address,
                    silent_payment_pubkey,
                    pubkey_hint,
                    descriptor_hint,
                    address_list,
                },
            },
            CorePaymentMethod::Lightning {
                label, lightning_address, lnurl, bolt12, receiver_pubkey,
            } => FfiPaymentMethod::Lightning {
                method: FfiLightningMethod {
                    label,
                    lightning_address,
                    lnurl,
                    bolt12,
                    receiver_pubkey,
                },
            },
            CorePaymentMethod::Ark {
                label, server, pubkey, vtxo_pointer, proof, expires_at, opaque_uri,
            } => FfiPaymentMethod::Ark {
                method: FfiArkMethod {
                    label,
                    server,
                    pubkey,
                    vtxo_pointer,
                    opaque_uri,
                    proof: proof.map(Into::into),
                    expires_at,
                },
            },
        }
    }
}

impl From<CoreArkOwnershipProof> for FfiArkOwnershipProof {
    fn from(p: CoreArkOwnershipProof) -> Self {
        FfiArkOwnershipProof {
            message: p.message,
            signature: p.signature,
            pubkey: p.pubkey,
        }
    }
}

impl From<CoreKeyRotation> for FfiKeyRotation {
    fn from(r: CoreKeyRotation) -> Self {
        FfiKeyRotation {
            previous_pubkey: r.previous_pubkey,
            new_pubkey: r.new_pubkey,
            authorization_signature: r.authorization_signature,
            rotated_at: r.rotated_at,
        }
    }
}

impl From<CoreMethodVerification> for FfiMethodVerification {
    fn from(v: CoreMethodVerification) -> Self {
        // Serialize the status to a simplified proof_type + proof_data for FFI
        let (proof_type, proof_data, verified_at) = match &v.status {
            satspath_core::ownership::VerificationStatus::Unverified => {
                ("unverified".to_string(), String::new(), 0i64)
            }
            satspath_core::ownership::VerificationStatus::Verified {
                proof_type, proof, verified_at, ..
            } => {
                let pt = format!("{:?}", proof_type);
                let pd = serde_json::to_string(proof).unwrap_or_default();
                (pt, pd, *verified_at)
            }
        };
        FfiMethodVerification {
            method_descriptor: v.method_descriptor,
            proof_type,
            proof_data,
            verified_at,
        }
    }
}

impl From<CorePaymentProfile> for FfiPaymentProfile {
    fn from(p: CorePaymentProfile) -> Self {
        FfiPaymentProfile {
            alias: p.alias,
            identity_pubkey: p.identity_pubkey,
            methods: p.methods.into_iter().map(Into::into).collect(),
            updated_at: p.updated_at,
            expires_at: p.expires_at,
            sequence: p.sequence,
            preferences: p.preferences,
            nonce: p.nonce,
            rotation: p.rotation.map(Into::into),
            method_verifications: p.method_verifications.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CoreSignedPaymentProfile> for FfiSignedPaymentProfile {
    fn from(sp: CoreSignedPaymentProfile) -> Self {
        FfiSignedPaymentProfile {
            profile: sp.profile.into(),
            signature: sp.signature,
        }
    }
}

impl From<CoreInvite> for FfiInvite {
    fn from(i: CoreInvite) -> Self {
        FfiInvite {
            alias_hash: i.alias_hash,
            amount_sats: i.amount_sats,
            created_at: i.created_at,
            expires_at: i.expires_at,
            claim_url: i.claim_url,
            warning: i.warning,
            sender_signature: i.sender_signature,
            sender_pubkey: i.sender_pubkey,
        }
    }
}

impl From<CoreInviteStatus> for FfiInviteStatus {
    fn from(s: CoreInviteStatus) -> Self {
        match s {
            CoreInviteStatus::Created => FfiInviteStatus::Created,
            CoreInviteStatus::EmailSent => FfiInviteStatus::EmailSent,
            CoreInviteStatus::ClaimedWithPublicProfile => FfiInviteStatus::ClaimedWithPublicProfile,
            CoreInviteStatus::Expired => FfiInviteStatus::Expired,
            CoreInviteStatus::Cancelled => FfiInviteStatus::Cancelled,
        }
    }
}

impl From<CoreInviteRecord> for FfiInviteRecord {
    fn from(r: CoreInviteRecord) -> Self {
        FfiInviteRecord {
            invite_id: r.invite_id,
            identifier_hash: r.identifier_hash,
            display_hint: r.display_hint,
            amount_sats: r.amount_sats,
            memo: r.memo,
            sender_fingerprint: r.sender_fingerprint,
            status: r.status.into(),
            created_at: r.created_at,
            expires_at: r.expires_at,
        }
    }
}

impl From<CoreExecutionMode> for FfiExecutionMode {
    fn from(m: CoreExecutionMode) -> Self {
        match m {
            CoreExecutionMode::Preview => FfiExecutionMode::Preview,
            CoreExecutionMode::MainnetPreview => FfiExecutionMode::MainnetPreview,
            CoreExecutionMode::TestnetExperimental => FfiExecutionMode::TestnetExperimental,
            CoreExecutionMode::ManualWallet => FfiExecutionMode::ManualWallet,
        }
    }
}

impl From<RouterFeeEstimate> for FfiFeeEstimate {
    fn from(f: RouterFeeEstimate) -> Self {
        FfiFeeEstimate {
            fastest_fee: f.fastest_fee,
            half_hour_fee: f.half_hour_fee,
            hour_fee: f.hour_fee,
            economy_fee: f.economy_fee,
            minimum_fee: f.minimum_fee,
        }
    }
}

impl From<CoreRouteQuote> for FfiRouteQuote {
    fn from(q: CoreRouteQuote) -> Self {
        FfiRouteQuote {
            selected_method: q.selected_method.into(),
            estimated_fee_sats: q.estimated_fee_sats.unwrap_or(0),
            estimated_confirmation: q.estimated_confirmation.unwrap_or_default(),
            reason: q.reason,
            execution: q.execution
                .map(Into::into)
                .unwrap_or(FfiExecutionMode::Preview),
            wallet_hint: q.wallet_hint.unwrap_or_default(),
        }
    }
}

// ── FFI → Core ────────────────────────────────────────────────────────────────

impl From<FfiPaymentMethod> for CorePaymentMethod {
    fn from(m: FfiPaymentMethod) -> Self {
        match m {
            FfiPaymentMethod::Onchain { method: m } => CorePaymentMethod::Onchain {
                label: m.label,
                network: parse_network(&m.network),
                address: m.address,
                silent_payment_pubkey: m.silent_payment_pubkey,
                pubkey_hint: m.pubkey_hint,
                descriptor_hint: m.descriptor_hint,
                address_list: m.address_list,
            },
            FfiPaymentMethod::Lightning { method: m } => CorePaymentMethod::Lightning {
                label: m.label,
                lightning_address: m.lightning_address,
                lnurl: m.lnurl,
                bolt12: m.bolt12,
                receiver_pubkey: m.receiver_pubkey,
            },
            FfiPaymentMethod::Ark { method: m } => CorePaymentMethod::Ark {
                label: m.label,
                server: m.server,
                pubkey: m.pubkey,
                vtxo_pointer: m.vtxo_pointer,
                opaque_uri: m.opaque_uri,
                proof: m.proof.map(Into::into),
                expires_at: m.expires_at,
            },
        }
    }
}

impl From<FfiArkOwnershipProof> for CoreArkOwnershipProof {
    fn from(p: FfiArkOwnershipProof) -> Self {
        CoreArkOwnershipProof {
            message: p.message,
            signature: p.signature,
            pubkey: p.pubkey,
        }
    }
}

impl From<FfiKeyRotation> for CoreKeyRotation {
    fn from(r: FfiKeyRotation) -> Self {
        CoreKeyRotation {
            previous_pubkey: r.previous_pubkey,
            new_pubkey: r.new_pubkey,
            authorization_signature: r.authorization_signature,
            rotated_at: r.rotated_at,
        }
    }
}

impl From<FfiPaymentProfile> for CorePaymentProfile {
    fn from(p: FfiPaymentProfile) -> Self {
        CorePaymentProfile {
            alias: p.alias,
            identity_pubkey: p.identity_pubkey,
            methods: p.methods.into_iter().map(Into::into).collect(),
            updated_at: p.updated_at,
            expires_at: p.expires_at,
            sequence: p.sequence,
            preferences: p.preferences,
            nonce: p.nonce,
            rotation: p.rotation.map(Into::into),
            method_verifications: p.method_verifications.into_iter().map(|v| {
                // Reconstruct a minimal CoreMethodVerification from FFI
                CoreMethodVerification {
                    method_descriptor: v.method_descriptor,
                    status: satspath_core::ownership::VerificationStatus::Unverified,
                }
            }).collect(),
        }
    }
}

impl From<FfiSignedPaymentProfile> for CoreSignedPaymentProfile {
    fn from(sp: FfiSignedPaymentProfile) -> Self {
        CoreSignedPaymentProfile {
            profile: sp.profile.into(),
            signature: sp.signature,
        }
    }
}

impl From<FfiFeeEstimate> for RouterFeeEstimate {
    fn from(f: FfiFeeEstimate) -> Self {
        RouterFeeEstimate {
            fastest_fee: f.fastest_fee,
            half_hour_fee: f.half_hour_fee,
            hour_fee: f.hour_fee,
            economy_fee: f.economy_fee,
            minimum_fee: f.minimum_fee,
        }
    }
}

impl From<FfiQuoteRequest> for satspath_router::RouteRequest {
    fn from(r: FfiQuoteRequest) -> Self {
        let urgency = match r.urgency.as_str() {
            "urgent" => PaymentUrgency::Urgent,
            "commercial" => PaymentUrgency::Commercial,
            "economy" => PaymentUrgency::Economy,
            _ => PaymentUrgency::Normal,
        };
        satspath_router::RouteRequest {
            alias: r.recipient,
            amount_sats: r.amount_sats,
            signed_profile: r.signed_profile.into(),
            urgency,
            max_fee_sats: r.max_fee_sats,
            max_fee_percent: r.max_fee_percent,
        }
    }
}

impl From<FfiSplitRecipient> for satspath_core::SplitRecipient {
    fn from(r: FfiSplitRecipient) -> Self {
        satspath_core::SplitRecipient {
            alias: r.alias,
            percent: r.percent,
        }
    }
}

impl From<FfiSplitPaymentRequest> for satspath_core::SplitPaymentRequest {
    fn from(r: FfiSplitPaymentRequest) -> Self {
        satspath_core::SplitPaymentRequest {
            version: r.version,
            total_amount_sats: r.total_amount_sats,
            splits: r.splits.into_iter().map(Into::into).collect(),
            memo: r.memo,
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_network(s: &str) -> BitcoinNetwork {
    match s {
        "testnet" => BitcoinNetwork::Testnet,
        "regtest" => BitcoinNetwork::Regtest,
        _ => BitcoinNetwork::Mainnet,
    }
}