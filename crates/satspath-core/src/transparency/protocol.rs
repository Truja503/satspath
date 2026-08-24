use serde::{Deserialize, Serialize};

use super::{
    MerkleConsistencyProof, MerkleInclusionProof, NameEvent, StateMapProof, TransparencyCheckpoint,
};
use crate::SignedPaymentProfile;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NamespaceDescriptor {
    pub version: u16,
    pub domain: String,
    pub log_id: String,
    pub authority_pubkey: String,
    pub endpoint_urls: Vec<String>,
    pub witness_quorum: u8,
    pub witness_pubkeys: Vec<String>,
    pub valid_from: i64,
    pub expires_at: i64,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WitnessCosignature {
    pub version: u16,
    pub witness_id: String,
    pub witness_pubkey: String,
    pub checkpoint_hash: String,
    pub tree_size: u64,
    pub timestamp: i64,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionEnvelope {
    pub version: u16,
    pub identifier: String,
    pub namespace_descriptor: NamespaceDescriptor,
    pub signed_profile: SignedPaymentProfile,
    pub name_events: Vec<NameEvent>,
    pub inclusion_proof: MerkleInclusionProof,
    pub checkpoint: TransparencyCheckpoint,
    pub consistency_proof: Option<MerkleConsistencyProof>,
    pub current_state_proof: Option<StateMapProof>,
    pub witness_cosignatures: Vec<WitnessCosignature>,
    pub served_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolutionRequest {
    pub identifier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_tree_size: Option<u64>,
    #[serde(default)]
    pub include_history: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EndpointRole {
    Primary,
    Replica,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplicaEndpoint {
    pub url: String,
    pub role: EndpointRole,
    pub priority: u8,
}
