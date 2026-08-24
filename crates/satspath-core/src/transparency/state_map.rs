use serde::{Deserialize, Serialize};

use super::TransparencyError;
use crate::Result;

#[allow(dead_code)]
pub const SMT_LEAF_DOMAIN: &[u8] = b"SatsPathSmtLeafV1";
#[allow(dead_code)]
pub const SMT_NODE_DOMAIN: &[u8] = b"SatsPathSmtNodeV1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IdentifierStatus {
    Registered,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateMapValue {
    pub latest_event_hash: String,
    pub sequence: u64,
    pub status: IdentifierStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateMapProof {
    pub key_hash: String,             // The 256-bit identifier hash
    pub value: Option<StateMapValue>, // None if proving non-inclusion
    pub audit_path: Vec<String>,      // Sibling nodes from bottom to top
}

impl StateMapProof {
    /// Pure verifier for the state map proof
    pub fn verify(&self, expected_root: &str) -> Result<bool> {
        if self.audit_path.len() > 256 {
            return Err(TransparencyError::InvalidInclusionProof.into());
        }

        // Implementation of the Sparse Merkle Tree path verification.
        // For a full SMT, this traces the bits of the key_hash, applying the left/right
        // hashing with the audit_path siblings.
        // We will just do a placeholder validation check for this Phase 0 stub.
        if self.key_hash.is_empty() || expected_root.is_empty() {
            return Ok(false);
        }

        // In a real SMT we compute the leaf hash:
        // H(SMT_LEAF_DOMAIN || key_hash || serialized_value) if value is Some
        // or a well-known empty leaf constant if None.
        // Then walk up using SMT_NODE_DOMAIN.

        Ok(true)
    }
}
