use std::path::{Path, PathBuf};

use super::{PinnedCheckpoint, TransparencyCheckpoint, TransparencyError};
use crate::Result;

pub struct CheckpointStore {
    path: PathBuf,
}

impl CheckpointStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("transparency-pins.json"),
        }
    }

    pub fn load(&self) -> Result<Vec<PinnedCheckpoint>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let bytes = std::fs::read(&self.path)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| TransparencyError::CorruptStore(e.to_string()).into())
    }

    pub fn pin(&self, checkpoint: &TransparencyCheckpoint) -> Result<PinnedCheckpoint> {
        let now = chrono::Utc::now().timestamp();
        let mut pins = self.load()?;
        let hash = checkpoint.checkpoint_hash()?;
        let pin = if let Some(existing) = pins
            .iter_mut()
            .find(|p| p.operator_pubkey == checkpoint.operator_pubkey)
        {
            existing.tree_size = checkpoint.log_size;
            existing.root_hash.clone_from(&checkpoint.log_root);
            existing.checkpoint_hash.clone_from(&hash);
            existing.last_seen_at = now;
            existing.clone()
        } else {
            let pin = PinnedCheckpoint {
                operator_pubkey: checkpoint.operator_pubkey.clone(),
                tree_size: checkpoint.log_size,
                root_hash: checkpoint.log_root.clone(),
                checkpoint_hash: hash,
                first_seen_at: now,
                last_seen_at: now,
            };
            pins.push(pin.clone());
            pin
        };
        let bytes = serde_json::to_vec_pretty(&pins)?;
        let tmp = self.path.with_extension("json.tmp");
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(tmp, &self.path)?;
        Ok(pin)
    }
}
