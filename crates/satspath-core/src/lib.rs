pub mod codec;
pub mod crypto;
pub mod errors;
pub mod peer_registry;
pub mod pointer;
pub mod privacy;
pub mod profile;
pub mod registry;
pub mod resolver;
pub mod resolvers;
pub mod validation;

pub use errors::{Result, SatsPathError};
pub use peer_registry::{
    canonicalize_identifier, display_hint, hash_identifier, LocalPeerRegistry, MockPeerRegistry,
    PeerPointers, PeerRecord, PeerRegistryBackend,
};
pub use pointer::{BitcoinNetwork, PaymentPointer};
pub use profile::{Invite, PaymentMethod, PaymentProfile, PaymentRequest, SignedPaymentProfile};

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
