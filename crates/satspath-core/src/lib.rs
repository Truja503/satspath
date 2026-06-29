pub mod codec;
pub mod crypto;
pub mod errors;
pub mod ownership;
pub mod peer_registry;
pub mod platform;
pub mod pointer;
pub mod privacy;
pub mod profile;
pub mod registry;
pub mod resolver;
pub mod resolvers;
pub mod validation;

pub use errors::{Result, SatsPathError};
pub use ownership::{
    build_manual_attestation, build_signature_attestation, ownership_challenge_message,
    pubkey_controls_address, stored_status_for_method, validate_method_verification,
    validate_ownership_proof, verify_method_verification, MethodVerification, OwnershipProof,
    ProofType, TrustTier, VerificationStatus,
};
pub use peer_registry::{
    canonicalize_identifier, display_hint, hash_identifier, LocalPeerRegistry, MockPeerRegistry,
    PeerPointers, PeerRecord, PeerRegistryBackend,
};
pub use platform::{
    EmailChallenge, EmailVerifier, ProfilePublisher, PublishReceipt, VerifiedIdentifier,
};
pub use pointer::{BitcoinNetwork, PaymentPointer};
pub use profile::{
    ClaimPolicy, Invite, InviteRecord, InviteStatus, PaymentMethod, PaymentProfile, PaymentRequest,
    SignedPaymentProfile,
};

use sha2::{Digest, Sha256};

/// Validate that a string looks like a Lightning Address (user@domain).
pub fn is_valid_lightning_address(s: &str) -> bool {
    validation::validate_lightning_address(s).is_ok()
}

/// Create an invite for an unregistered alias.
pub fn create_invite(alias: &str, amount_sats: u64) -> Invite {
    let digest = Sha256::digest(privacy::canonical_identifier(alias).as_bytes());
    let alias_hash = hex::encode(digest);
    let claim_url = format!(
        "https://satspath.local/claim?alias_hash={}&amount={}",
        &alias_hash[..16],
        amount_sats
    );
    Invite {
        alias_hash,
        amount_sats,
        created_at: chrono::Utc::now().timestamp(),
        claim_url,
        warning: "The receiver must claim this payment by generating their own keys locally. \
                  SatsPath never holds or generates keys on behalf of users."
            .into(),
    }
}

pub fn create_invite_record(
    identifier: &str,
    amount_sats: u64,
    memo: Option<String>,
    sender_fingerprint: String,
    ttl_seconds: i64,
) -> InviteRecord {
    let now = chrono::Utc::now().timestamp();
    InviteRecord {
        invite_id: uuid::Uuid::new_v4().to_string(),
        identifier_hash: privacy::identifier_hash(identifier),
        display_hint: privacy::mask_identifier(identifier),
        amount_sats,
        memo,
        sender_fingerprint,
        status: InviteStatus::Created,
        created_at: now,
        expires_at: now + ttl_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_user_invite_record_contains_no_private_material() {
        let invite = create_invite_record(
            "someone@gmail.com",
            1_000,
            Some("coffee".into()),
            "sender-fp".into(),
            600,
        );
        assert_eq!(invite.status, InviteStatus::Created);
        assert_eq!(invite.display_hint, "s***@gmail.com");
        assert!(!format!("{invite:?}").contains("seed"));
        assert!(!format!("{invite:?}").contains("xprv"));
        assert!(invite.expires_at > invite.created_at);
    }
}
