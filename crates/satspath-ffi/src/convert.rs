//! Conversion between internal and FFI types

use satspath_core::{
    SignedPaymentProfile as CoreSignedPaymentProfile,
    PaymentProfile as CorePaymentProfile,
    PaymentMethod as CorePaymentMethod,
    Invite as CoreInvite,
    InviteRecord as CoreInviteRecord,
    Identity as CoreIdentity,
    FeeEstimate as CoreFeeEstimate,
};
use satspath_router::{
    RouteQuote as CoreRouteQuote,
    FeeEstimate as RouterFeeEstimate,
    urgency::PaymentUrgency,
};

// Conversion from internal types to generated FFI types
impl From<CoreIdentity> for crate::satspath::r#Identity {
    fn from(internal: CoreIdentity) -> Self {
        crate::satspath::r#Identity {
            pubkeyHex: internal.pubkey_hex,
            secretKeyPath: internal.secret_key_path,
            fingerprint: internal.fingerprint,
        }
    }
}

impl From<CoreSignedPaymentProfile> for crate::satspath::r#SignedPaymentProfile {
    fn from(internal: CoreSignedPaymentProfile) -> Self {
        crate::satspath::r#SignedPaymentProfile {
            profile: internal.profile.into(),
            signature: internal.signature,
        }
    }
}

impl From<CorePaymentProfile> for crate::satspath::r#PaymentProfile {
    fn from(internal: CorePaymentProfile) -> Self {
        crate::satspath::r#PaymentProfile {
            alias: internal.alias,
            identityPubkey: internal.identity_pubkey,
            methods: internal.methods.into_iter().map(Into::into).collect(),
            updatedAt: internal.updated_at,
            expiresAt: internal.expires_at,
            sequence: internal.sequence,
            preferences: internal.preferences,
            nonce: internal.nonce,
            rotation: internal.rotation.map(Into::into),
            methodVerifications: internal.method_verifications.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CorePaymentMethod> for crate::satspath::r#PaymentMethod {
    fn from(internal: CorePaymentMethod) -> Self {
        match internal {
            CorePaymentMethod::Onchain { label, network, address, silent_payment_pubkey, pubkey_hint, descriptor_hint, address_list } => {
                crate::satspath::r#PaymentMethod::OnchainMethod(crate::satspath::r#OnchainMethod {
                    label,
                    network: format!("{:?}", network).to_lowercase(),
                    address,
                    silentPaymentPubkey: silent_payment_pubkey,
                    pubkeyHint: pubkey_hint,
                    descriptorHint: descriptor_hint,
                    addressList: address_list,
                })
            }
            CorePaymentMethod::Lightning { label, lightning_address, lnurl, bolt12, receiver_pubkey } => {
                crate::satspath::r#PaymentMethod::LightningMethod(crate::satspath::r#LightningMethod {
                    label,
                    lightningAddress: lightning_address,
                    lnurl,
                    bolt12,
                    receiverPubkey: receiver_pubkey,
                })
            }
            CorePaymentMethod::Ark { label, server, pubkey, vtxo_pointer, proof, expires_at, opaque_uri } => {
                crate::satspath::r#PaymentMethod::ArkMethod(crate::satspath::r#ArkMethod {
                    label,
                    server,
                    pubkey,
                    vtxoPointer: vtxo_pointer,
                    proof: proof.map(Into::into),
                    expiresAt: expires_at,
                    opaqueUri: opaque_uri,
                })
            }
        }
    }
}

impl From<satspath_core::ark::ArkOwnershipProof> for crate::satspath::r#ArkOwnershipProof {
    fn from(internal: satspath_core::ark::ArkOwnershipProof) -> Self {
        crate::satspath::r#ArkOwnershipProof {
            proofType: internal.proof_type,
            proofData: internal.proof_data,
            timestamp: internal.timestamp,
        }
    }
}

impl From<CoreInvite> for crate::satspath::r#Invite {
    fn from(internal: CoreInvite) -> Self {
        crate::satspath::r#Invite {
            aliasHash: internal.alias_hash,
            amountSats: internal.amount_sats,
            createdAt: internal.created_at,
            expiresAt: internal.expires_at,
            claimUrl: internal.claim_url,
            warning: internal.warning,
            senderSignature: internal.sender_signature,
            senderPubkey: internal.sender_pubkey,
        }
    }
}

impl From<CoreInviteRecord> for crate::satspath::r#InviteRecord {
    fn from(internal: CoreInviteRecord) -> Self {
        crate::satspath::r#InviteRecord {
            inviteId: internal.invite_id,
            identifierHash: internal.identifier_hash,
            displayHint: internal.display_hint,
            amountSats: internal.amount_sats,
            memo: internal.memo,
            senderFingerprint: internal.sender_fingerprint,
            status: match internal.status {
                satspath_core::InviteStatus::Created => crate::satspath::r#InviteStatus::Created,
                satspath_core::InviteStatus::EmailSent => crate::satspath::r#InviteStatus::EmailSent,
                satspath_core::InviteStatus::ClaimedWithPublicProfile => crate::satspath::r#InviteStatus::ClaimedWithPublicProfile,
                satspath_core::InviteStatus::Expired => crate::satspath::r#InviteStatus::Expired,
                satspath_core::InviteStatus::Cancelled => crate::satspath::r#InviteStatus::Cancelled,
            },
            createdAt: internal.created_at,
            expiresAt: internal.expires_at,
        }
    }
}

impl From<satspath_core::FeeEstimate> for crate::satspath::r#FeeEstimate {
    fn from(internal: satspath_core::FeeEstimate) -> Self {
        crate::satspath::r#FeeEstimate {
            fastestFee: internal.fastest_fee,
            halfHourFee: internal.half_hour_fee,
            hourFee: internal.hour_fee,
            economyFee: internal.economy_fee,
            minimumFee: internal.minimum_fee,
        }
    }
}

impl From<satspath_router::FeeEstimate> for crate::satspath::r#FeeEstimate {
    fn from(internal: satspath_router::FeeEstimate) -> Self {
        crate::satspath::r#FeeEstimate {
            fastestFee: internal.fastest_fee,
            halfHourFee: internal.half_hour_fee,
            hourFee: internal.hour_fee,
            economyFee: internal.economy_fee,
            minimumFee: internal.minimum_fee,
        }
    }
}

impl From<crate::satspath::r#FeeEstimate> for satspath_router::FeeEstimate {
    fn from(external: crate::satspath::r#FeeEstimate) -> Self {
        satspath_router::FeeEstimate {
            fastest_fee: external.fastestFee,
            half_hour_fee: external.halfHourFee,
            hour_fee: external.hourFee,
            economy_fee: external.economyFee,
            minimum_fee: external.minimumFee,
        }
    }
}

impl From<satspath_router::RouteQuote> for crate::satspath::r#RouteQuote {
    fn from(internal: satspath_router::RouteQuote) -> Self {
        crate::satspath::r#RouteQuote {
            selected_method: internal.selected_method.into(),
            estimated_fee_sats: internal.estimated_fee_sats.unwrap_or(0),
            estimated_confirmation: internal.estimated_confirmation.unwrap_or_default(),
            reason: internal.reason,
            execution: match internal.execution {
                Some(satspath_core::ExecutionMode::Preview) => crate::satspath::r#ExecutionMode::Preview,
                Some(satspath_core::ExecutionMode::MainnetPreview) => crate::satspath::r#ExecutionMode::MainnetPreview,
                Some(satspath_core::ExecutionMode::TestnetExperimental) => crate::satspath::r#ExecutionMode::TestnetExperimental,
                Some(satspath_core::ExecutionMode::ManualWallet) => crate::satspath::r#ExecutionMode::ManualWallet,
                None => crate::satspath::r#ExecutionMode::Preview,
            },
            wallet_hint: internal.wallet_hint.unwrap_or_default(),
        }
    }
}

impl From<satspath_core::KeyRotation> for crate::satspath::r#KeyRotation {
    fn from(internal: satspath_core::KeyRotation) -> Self {
        crate::satspath::r#KeyRotation {
            newIdentityPubkey: internal.new_identity_pubkey,
            rotationTime: internal.rotation_time,
            previousSignature: internal.previous_signature,
        }
    }
}

impl From<satspath_core::ownership::MethodVerification> for crate::satspath::r#MethodVerification {
    fn from(internal: satspath_core::ownership::MethodVerification) -> Self {
        crate::satspath::r#MethodVerification {
            methodDescriptor: internal.method_descriptor,
            proofType: internal.proof_type,
            proofData: internal.proof_data,
            verifiedAt: internal.verified_at,
        }
    }
}

impl From<satspath_router::urgency::PaymentUrgency> for crate::satspath::r#ExecutionMode {
    fn from(internal: satspath_router::urgency::PaymentUrgency) -> Self {
        match internal {
            satspath_router::urgency::PaymentUrgency::Urgent => crate::satspath::r#ExecutionMode::Preview,
            satspath_router::urgency::PaymentUrgency::Commercial => crate::satspath::r#ExecutionMode::MainnetPreview,
            satspath_router::urgency::PaymentUrgency::Economy => crate::satspath::r#ExecutionMode::TestnetExperimental,
            satspath_router::urgency::PaymentUrgency::Normal => crate::satspath::r#ExecutionMode::Preview,
        }
    }
}

impl From<crate::satspath::r#PaymentMethod> for satspath_core::PaymentMethod {
    fn from(external: crate::satspath::r#PaymentMethod) -> Self {
        match external {
            crate::satspath::r#PaymentMethod::OnchainMethod(m) => {
                satspath_core::PaymentMethod::Onchain {
                    label: m.label,
                    network: m.network.parse().unwrap_or(satspath_core::pointer::BitcoinNetwork::Mainnet),
                    address: m.address,
                    silent_payment_pubkey: m.silent_payment_pubkey,
                    pubkey_hint: m.pubkey_hint,
                    descriptor_hint: m.descriptor_hint,
                    address_list: m.address_list,
                }
            }
            crate::satspath::r#PaymentMethod::LightningMethod(m) => {
                satspath_core::PaymentMethod::Lightning {
                    label: m.label,
                    lightning_address: m.lightning_address,
                    lnurl: m.lnurl,
                    bolt12: m.bolt12,
                    receiver_pubkey: m.receiver_pubkey,
                }
            }
            crate::satspath::r#PaymentMethod::ArkMethod(m) => {
                satspath_core::PaymentMethod::Ark {
                    label: m.label,
                    server: m.server,
                    pubkey: m.pubkey,
                    vtxo_pointer: m.vtxo_pointer,
                    proof: m.proof.map(Into::into),
                    expires_at: m.expires_at,
                    opaque_uri: m.opaque_uri,
                }
            }
        }
    }
}

impl From<crate::satspath::r#PaymentProfile> for satspath_core::PaymentProfile {
    fn from(external: crate::satspath::r#PaymentProfile) -> Self {
        satspath_core::PaymentProfile {
            alias: external.alias,
            identity_pubkey: external.identityPubkey,
            methods: external.methods.into_iter().map(Into::into).collect(),
            updated_at: external.updatedAt,
            expires_at: external.expiresAt,
            sequence: external.sequence,
            preferences: external.preferences,
            nonce: external.nonce,
            rotation: external.rotation.map(Into::into),
            method_verifications: external.methodVerifications.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<crate::satspath::r#PaymentMethod> for satspath_core::PaymentMethod {
    fn from(external: crate::satspath::r#PaymentMethod) -> Self {
        match external {
            crate::satspath::r#PaymentMethod::OnchainMethod(m) => {
                satspath_core::PaymentMethod::Onchain {
                    label: m.label,
                    network: m.network.parse().unwrap_or(satspath_core::pointer::BitcoinNetwork::Mainnet),
                    address: m.address,
                    silent_payment_pubkey: m.silent_payment_pubkey,
                    pubkey_hint: m.pubkeyHint,
                    descriptor_hint: m.descriptorHint,
                    address_list: m.addressList,
                }
            }
            crate::satspath::r#PaymentMethod::LightningMethod(m) => {
                satspath_core::PaymentMethod::Lightning {
                    label: m.label,
                    lightning_address: m.lightningAddress,
                    lnurl: m.lnurl,
                    bolt12: m.bolt12,
                    receiver_pubkey: m.receiverPubkey,
                }
            }
            crate::satspath::r#PaymentMethod::ArkMethod(m) => {
                satspath_core::PaymentMethod::Ark {
                    label: m.label,
                    server: m.server,
                    pubkey: m.pubkey,
                    vtxo_pointer: m.vtxoPointer,
                    proof: m.proof.map(Into::into),
                    expires_at: m.expiresAt,
                    opaque_uri: m.opaqueUri,
                }
            }
        }
    }
}

impl From<crate::satspath::r#QuoteRequest> for satspath_router::RouteRequest {
    fn from(external: crate::satspath::r#QuoteRequest) -> Self {
        let urgency = match external.urgency.as_str() {
            "urgent" => satspath_router::urgency::PaymentUrgency::Urgent,
            "commercial" => satspath_router::urgency::PaymentUrgency::Commercial,
            "economy" => satspath_router::urgency::PaymentUrgency::Economy,
            _ => satspath_router::urgency::PaymentUrgency::Normal,
        };

        satspath_router::RouteRequest {
            alias: external.recipient,
            amount_sats: external.amountSats,
            signed_profile: external.signedProfile.into(),
            urgency,
            max_fee_sats: external.maxFeeSats,
            max_fee_percent: external.maxFeePercent,
        }
    }
}

impl From<satspath_core::ark::ArkOwnershipProof> for crate::satspath::r#ArkOwnershipProof {
    fn from(internal: satspath_core::ark::ArkOwnershipProof) -> Self {
        crate::satspath::r#ArkOwnershipProof {
            proofType: internal.proof_type,
            proofData: internal.proof_data,
            timestamp: internal.timestamp,
        }
    }
}

impl From<satspath_core::InviteStatus> for crate::satspath::r#InviteStatus {
    fn from(internal: satspath_core::InviteStatus) -> Self {
        match internal {
            satspath_core::InviteStatus::Created => crate::satspath::r#InviteStatus::Created,
            satspath_core::InviteStatus::EmailSent => crate::satspath::r#InviteStatus::EmailSent,
            satspath_core::InviteStatus::ClaimedWithPublicProfile => crate::satspath::r#InviteStatus::ClaimedWithPublicProfile,
            satspath_core::InviteStatus::Expired => crate::satspath::r#InviteStatus::Expired,
            satspath_core::InviteStatus::Cancelled => crate::satspath::r#InviteStatus::Cancelled,
        }
    }
}

impl From<satspath_core::ExecutionMode> for crate::satspath::r#ExecutionMode {
    fn from(internal: satspath_core::ExecutionMode) -> Self {
        match internal {
            satspath_core::ExecutionMode::Preview => crate::satspath::r#ExecutionMode::Preview,
            satspath_core::ExecutionMode::MainnetPreview => crate::satspath::r#ExecutionMode::MainnetPreview,
            satspath_core::ExecutionMode::TestnetExperimental => crate::satspath::r#ExecutionMode::TestnetExperimental,
            satspath_core::ExecutionMode::ManualWallet => crate::satspath::r#ExecutionMode::ManualWallet,
        }
    }
}