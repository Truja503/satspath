use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::crypto::sign_message;
use crate::Result;

use super::BitcoinAnchor;

pub const CHECKPOINT_DOMAIN: &str = "SatsPathTransparencyCheckpointV1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransparencyCheckpoint {
    pub version: u16,
    pub log_size: u64,
    pub log_root: String,
    pub map_root: Option<String>,
    pub previous_checkpoint_hash: Option<String>,
    pub created_at: i64,
    pub operator_pubkey: String,
    pub operator_signature: String,
    pub bitcoin_anchor: Option<BitcoinAnchor>,
}

impl TransparencyCheckpoint {
    pub fn signing_message(&self) -> Result<String> {
        let mut unsigned = self.clone();
        unsigned.operator_signature.clear();
        let value = serde_json::to_value(unsigned)?;
        let canonical = canonical_json::to_string(&value)
            .map_err(|e| crate::SatsPathError::SerializationError(e.to_string()))?;
        Ok(format!("{CHECKPOINT_DOMAIN}\n{canonical}"))
    }

    pub fn sign(&mut self, secret_key: &SecretKey) -> Result<()> {
        self.operator_signature = sign_message(&self.signing_message()?, secret_key);
        Ok(())
    }

    pub fn checkpoint_hash(&self) -> Result<String> {
        // Stable anchor identity: the Bitcoin receipt is deliberately excluded
        // to avoid a circular txid -> checkpoint -> txid commitment. The
        // operator signature still commits to the receipt via signing_message.
        let mut core = self.clone();
        core.operator_signature.clear();
        core.bitcoin_anchor = None;
        let canonical = canonical_json::to_string(&serde_json::to_value(core)?)
            .map_err(|e| crate::SatsPathError::SerializationError(e.to_string()))?;
        let mut h = Sha256::new();
        h.update(b"SatsPathTransparencyCheckpointHashV1");
        h.update(canonical.as_bytes());
        Ok(hex::encode(h.finalize()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PinnedCheckpoint {
    pub operator_pubkey: String,
    pub tree_size: u64,
    pub root_hash: String,
    pub checkpoint_hash: String,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointComparison {
    FirstSeen,
    Unchanged,
    ConsistentExtension,
}
