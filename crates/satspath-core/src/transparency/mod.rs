mod anchor;
mod attestation;
mod checkpoint;
#[cfg(feature = "std")]
mod database;
mod event;
mod log;
mod proof;
mod store;
mod tree;
mod verifier;

#[cfg(feature = "std")]
pub use anchor::RegtestAnchorClient;
pub use anchor::{anchor_commitment, BitcoinAnchor, MockAnchorClient};
pub use attestation::{
    verify_attestation_binding, IdentifierAttestation, IdentifierVerificationMethod,
    TrustedVerifier,
};
pub use checkpoint::{
    CheckpointComparison, OperatorKeyRotation, PinnedCheckpoint, TransparencyCheckpoint,
    TransparencyLogIdentity,
};
#[cfg(feature = "std")]
pub use database::TransactionalTransparencyStore;
pub use event::{payment_method_descriptor_hash, profile_hash, NameAction, NameEvent};
pub use log::{ConsistencyStatus, TransparencyLog, TransparencyStatus};
pub use proof::{MerkleConsistencyProof, MerkleInclusionProof};
pub use store::CheckpointStore;
pub use tree::{leaf_hash, merkle_root, node_hash};
pub use verifier::{
    next_identifier_sequence, verify_checkpoint, verify_checkpoint_inclusion,
    verify_checkpoint_transition, verify_consistency_proof, verify_event_profile,
    verify_identifier_history, verify_inclusion_proof, verify_key_continuity,
};

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransparencyError {
    #[error("conflicting checkpoint for the same operator and tree size")]
    ConflictingCheckpoint,
    #[error("checkpoint rollback detected")]
    CheckpointRollback,
    #[error("invalid checkpoint signature")]
    InvalidCheckpointSignature,
    #[error("invalid consistency proof")]
    InvalidConsistencyProof,
    #[error("invalid inclusion proof")]
    InvalidInclusionProof,
    #[error("inclusion proof is not bound to the signed checkpoint")]
    CheckpointInclusionMismatch,
    #[error("unexpected transparency operator key")]
    UnexpectedOperatorKey,
    #[error("invalid transparency operator rotation")]
    InvalidOperatorRotation,
    #[error("corrupt checkpoint chain: {0}")]
    CorruptCheckpointChain(String),
    #[error("broken identifier history: {0}")]
    BrokenIdentifierHistory(String),
    #[error("unauthorized key replacement")]
    UnauthorizedKeyReplacement,
    #[error("invalid rotation: {0}")]
    InvalidRotation(String),
    #[error("profile hash mismatch")]
    ProfileHashMismatch,
    #[error("invalid event signature")]
    InvalidEventSignature,
    #[error("duplicate registration")]
    DuplicateRegistration,
    #[error("identifier is revoked")]
    IdentifierRevoked,
    #[error("recovery is disabled")]
    RecoveryDisabled,
    #[error("corrupt transparency store: {0}")]
    CorruptStore(String),
    #[error("invalid identifier attestation: {0}")]
    InvalidAttestation(String),
}
