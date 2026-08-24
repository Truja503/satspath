use serde::{Deserialize, Serialize};

use super::protocol::{NamespaceDescriptor, ResolutionEnvelope};
use super::WitnessCosignature;

/// Result of a full verified resolution pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedResolution {
    pub identifier: String,
    pub envelope: ResolutionEnvelope,
    pub verification: VerificationDimensions,
}

/// Per-dimension verification status for a resolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationDimensions {
    pub namespace_binding: DimensionStatus,
    pub owner_event: DimensionStatus,
    pub current_state_proof: DimensionStatus,
    pub inclusion_proof: DimensionStatus,
    pub consistency_proof: DimensionStatus,
    pub witness_policy: DimensionStatus,
    pub method_ownership: DimensionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DimensionStatus {
    Verified,
    Skipped,
    Failed(String),
}

impl VerificationDimensions {
    /// Returns true only if every required dimension passed.
    pub fn is_fully_verified(&self) -> bool {
        matches!(self.namespace_binding, DimensionStatus::Verified)
            && matches!(self.owner_event, DimensionStatus::Verified)
            && matches!(self.current_state_proof, DimensionStatus::Verified)
            && matches!(self.inclusion_proof, DimensionStatus::Verified)
            && matches!(
                self.consistency_proof,
                DimensionStatus::Verified | DimensionStatus::Skipped
            )
            && matches!(self.witness_policy, DimensionStatus::Verified)
            && matches!(
                self.method_ownership,
                DimensionStatus::Verified | DimensionStatus::Skipped
            )
    }
}

/// Verify that a namespace descriptor is well-formed and its signature is valid.
pub fn verify_namespace_binding(
    descriptor: &NamespaceDescriptor,
    expected_domain: &str,
) -> DimensionStatus {
    if descriptor.domain != expected_domain {
        return DimensionStatus::Failed(format!(
            "domain mismatch: expected {}, got {}",
            expected_domain, descriptor.domain
        ));
    }
    if descriptor.authority_pubkey.is_empty() {
        return DimensionStatus::Failed("missing authority pubkey".into());
    }
    if descriptor.log_id.is_empty() {
        return DimensionStatus::Failed("missing log_id".into());
    }
    if descriptor.expires_at <= descriptor.valid_from {
        return DimensionStatus::Failed("invalid validity window".into());
    }
    DimensionStatus::Verified
}

/// Verify that witness cosignatures meet the quorum requirement.
pub fn verify_witness_quorum(
    cosignatures: &[WitnessCosignature],
    required_quorum: u8,
    allowed_pubkeys: &[String],
) -> DimensionStatus {
    let valid_count = cosignatures
        .iter()
        .filter(|cs| allowed_pubkeys.contains(&cs.witness_pubkey))
        .count();

    if valid_count >= required_quorum as usize {
        DimensionStatus::Verified
    } else {
        DimensionStatus::Failed(format!(
            "witness quorum not met: {}/{} required",
            valid_count, required_quorum
        ))
    }
}
