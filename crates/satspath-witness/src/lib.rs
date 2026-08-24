use async_trait::async_trait;
use satspath_core::transparency::{
    MerkleConsistencyProof, TransparencyCheckpoint, WitnessCosignature,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WitnessError {
    #[error("Checkpoint signature invalid")]
    InvalidSignature,
    #[error("Tree size rolled back from {pinned} to {proposed}")]
    Rollback { pinned: u64, proposed: u64 },
    #[error("Equivocation detected for log {log_id} at tree size {tree_size}")]
    EquivocationDetected { log_id: String, tree_size: u64 },
    #[error("Consistency proof verification failed")]
    InvalidConsistencyProof,
    #[error("Timestamp outside acceptable freshness window")]
    StaleTimestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedState {
    pub log_id: String,
    pub tree_size: u64,
    pub root_hash: String,
    pub signature: String,
    pub timestamp: i64,
}

#[async_trait]
pub trait PinStore {
    async fn get_pin(&self, log_id: &str) -> Result<Option<PinnedState>, WitnessError>;
    async fn save_pin(&self, pin: PinnedState) -> Result<(), WitnessError>;
    async fn record_equivocation(
        &self,
        log_id: &str,
        tree_size: u64,
        proof_a: PinnedState,
        proof_b: PinnedState,
    ) -> Result<(), WitnessError>;
}

pub struct WitnessService<S: PinStore> {
    pub witness_id: String,
    pub store: S,
}

impl<S: PinStore> WitnessService<S> {
    pub fn new(witness_id: String, store: S) -> Self {
        Self { witness_id, store }
    }

    pub async fn process_checkpoint(
        &self,
        checkpoint: &TransparencyCheckpoint,
        consistency_proof: Option<&MerkleConsistencyProof>,
    ) -> Result<WitnessCosignature, WitnessError> {
        let pinned = self.store.get_pin(&checkpoint.log_id).await?;

        if let Some(pin) = pinned {
            if checkpoint.log_size < pin.tree_size {
                return Err(WitnessError::Rollback {
                    pinned: pin.tree_size,
                    proposed: checkpoint.log_size,
                });
            }

            if checkpoint.log_size == pin.tree_size && checkpoint.log_root != pin.root_hash {
                let equiv_a = pin.clone();
                let equiv_b = PinnedState {
                    log_id: checkpoint.log_id.clone(),
                    tree_size: checkpoint.log_size,
                    root_hash: checkpoint.log_root.clone(),
                    signature: checkpoint.operator_signature.clone(),
                    timestamp: checkpoint.created_at,
                };
                self.store
                    .record_equivocation(&checkpoint.log_id, checkpoint.log_size, equiv_a, equiv_b)
                    .await?;
                return Err(WitnessError::EquivocationDetected {
                    log_id: checkpoint.log_id.clone(),
                    tree_size: checkpoint.log_size,
                });
            }

            if checkpoint.log_size > pin.tree_size {
                let _proof = consistency_proof.ok_or(WitnessError::InvalidConsistencyProof)?;
                // In a real implementation: verify_consistency_proof(&pin.root_hash, checkpoint, proof)?
            }
        }

        // Checkpoint signature and timestamp policy should be validated here before saving.

        let new_pin = PinnedState {
            log_id: checkpoint.log_id.clone(),
            tree_size: checkpoint.log_size,
            root_hash: checkpoint.log_root.clone(),
            signature: checkpoint.operator_signature.clone(),
            timestamp: checkpoint.created_at,
        };
        self.store.save_pin(new_pin).await?;

        Ok(WitnessCosignature {
            version: 1,
            witness_id: self.witness_id.clone(),
            witness_pubkey: "dummy_pubkey".to_string(), // In real impl, use actual key
            checkpoint_hash: "dummy_hash".to_string(),
            tree_size: checkpoint.log_size,
            timestamp: chrono::Utc::now().timestamp(),
            signature: "dummy_sig".to_string(),
        })
    }
}
