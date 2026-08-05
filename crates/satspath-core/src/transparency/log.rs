use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use secp256k1::{PublicKey, Secp256k1, SecretKey};
use serde::{Deserialize, Serialize};

use crate::transparency::event::profile_hash;
use crate::{Result, SignedPaymentProfile};

use super::tree::inclusion_proof;
use super::{
    anchor_commitment, leaf_hash, merkle_root, verify_identifier_history, BitcoinAnchor,
    MerkleConsistencyProof, MerkleInclusionProof, NameAction, NameEvent, TransparencyCheckpoint,
    TransparencyError,
};

const EVENTS_FILE: &str = "transparency-events-v1.jsonl";
const CHECKPOINTS_FILE: &str = "transparency-checkpoints-v1.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransparencyStatus {
    pub log_size: u64,
    pub log_root: String,
    pub latest_checkpoint_hash: Option<String>,
    pub registered_identifiers: u64,
    pub key_rotations: u64,
    pub revocations: u64,
    pub map_root: Option<String>,
    pub consistency_status: String,
}

pub struct TransparencyLog {
    dir: PathBuf,
    events: Vec<NameEvent>,
    checkpoints: Vec<TransparencyCheckpoint>,
}

impl TransparencyLog {
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let events_path = dir.join(EVENTS_FILE);
        let events = if events_path.exists() {
            let raw = std::fs::read_to_string(&events_path)?;
            raw.lines()
                .enumerate()
                .filter(|(_, line)| !line.trim().is_empty())
                .map(|(i, line)| {
                    serde_json::from_str(line).map_err(|e| {
                        TransparencyError::CorruptStore(format!("event line {}: {e}", i + 1)).into()
                    })
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        let checkpoints_path = dir.join(CHECKPOINTS_FILE);
        let checkpoints = if checkpoints_path.exists() {
            serde_json::from_slice(&std::fs::read(checkpoints_path)?)
                .map_err(|e| TransparencyError::CorruptStore(format!("checkpoints: {e}")))?
        } else {
            Vec::new()
        };
        let log = Self {
            dir: dir.to_owned(),
            events,
            checkpoints,
        };
        log.verify_replay()?;
        Ok(log)
    }

    pub fn events(&self) -> &[NameEvent] {
        &self.events
    }
    pub fn checkpoints(&self) -> &[TransparencyCheckpoint] {
        &self.checkpoints
    }

    pub fn event(&self, hash: &str) -> Result<Option<&NameEvent>> {
        for event in &self.events {
            if event.event_hash()? == hash {
                return Ok(Some(event));
            }
        }
        Ok(None)
    }

    pub fn history(&self, identifier_hash: &str) -> Vec<&NameEvent> {
        self.events
            .iter()
            .filter(|e| e.identifier_hash == identifier_hash)
            .collect()
    }

    pub fn append(&mut self, event: NameEvent, profile: &SignedPaymentProfile) -> Result<String> {
        if event.version != 1 || event.profile_hash != profile_hash(profile)? {
            return Err(TransparencyError::ProfileHashMismatch.into());
        }
        let mut history: Vec<NameEvent> = self
            .history(&event.identifier_hash)
            .into_iter()
            .cloned()
            .collect();
        history.push(event.clone());
        verify_identifier_history(&history)?;
        let event_hash = event.event_hash()?;
        let encoded = serde_json::to_vec(&event)?;
        let path = self.dir.join(EVENTS_FILE);
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        file.write_all(&encoded)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        self.events.push(event);
        Ok(event_hash)
    }

    pub fn leaf_hashes(&self, size: usize) -> Result<Vec<[u8; 32]>> {
        if size > self.events.len() {
            return Err(TransparencyError::InvalidConsistencyProof.into());
        }
        self.events[..size]
            .iter()
            .map(|e| {
                Ok(leaf_hash(&hex::decode(e.event_hash()?).map_err(|_| {
                    TransparencyError::CorruptStore("event hash".into())
                })?))
            })
            .collect()
    }

    pub fn inclusion(
        &self,
        event_hash: &str,
        tree_size: Option<u64>,
    ) -> Result<MerkleInclusionProof> {
        let size = tree_size.unwrap_or(self.events.len() as u64) as usize;
        let index = self.events[..size]
            .iter()
            .position(|e| e.event_hash().ok().as_deref() == Some(event_hash))
            .ok_or(TransparencyError::InvalidInclusionProof)?;
        inclusion_proof(&self.leaf_hashes(size)?, index).map_err(Into::into)
    }

    pub fn consistency(&self, old_size: u64, new_size: u64) -> Result<MerkleConsistencyProof> {
        if old_size == 0 || old_size > new_size || new_size > self.events.len() as u64 {
            return Err(TransparencyError::InvalidConsistencyProof.into());
        }
        let leaves = self.leaf_hashes(new_size as usize)?;
        Ok(MerkleConsistencyProof {
            old_tree_size: old_size,
            new_tree_size: new_size,
            old_root: hex::encode(merkle_root(&leaves[..old_size as usize])),
            new_root: hex::encode(merkle_root(&leaves)),
            proof: leaves.iter().map(hex::encode).collect(),
        })
    }

    pub fn create_checkpoint(
        &mut self,
        operator_key: &SecretKey,
    ) -> Result<TransparencyCheckpoint> {
        if self.events.is_empty() {
            return Err(
                TransparencyError::CorruptStore("cannot checkpoint an empty log".into()).into(),
            );
        }
        let secp = Secp256k1::new();
        let operator_pubkey =
            hex::encode(PublicKey::from_secret_key(&secp, operator_key).serialize());
        let mut checkpoint = TransparencyCheckpoint {
            version: 1,
            log_size: self.events.len() as u64,
            log_root: hex::encode(merkle_root(&self.leaf_hashes(self.events.len())?)),
            map_root: None,
            previous_checkpoint_hash: self
                .checkpoints
                .last()
                .map(TransparencyCheckpoint::checkpoint_hash)
                .transpose()?,
            created_at: chrono::Utc::now().timestamp(),
            operator_pubkey,
            operator_signature: String::new(),
            bitcoin_anchor: None,
        };
        checkpoint.sign(operator_key)?;
        self.checkpoints.push(checkpoint.clone());
        self.save_checkpoints()?;
        Ok(checkpoint)
    }

    pub fn attach_latest_anchor(
        &mut self,
        anchor: BitcoinAnchor,
        operator_key: &SecretKey,
    ) -> Result<TransparencyCheckpoint> {
        let checkpoint = self
            .checkpoints
            .last_mut()
            .ok_or_else(|| TransparencyError::CorruptStore("no checkpoint to anchor".into()))?;
        if anchor.network != crate::BitcoinNetwork::Regtest
            || anchor.commitment
                != anchor_commitment(&checkpoint.checkpoint_hash()?)
                    .map_err(|e| crate::SatsPathError::SerializationError(e.to_string()))?
        {
            return Err(TransparencyError::CorruptStore(
                "anchor commitment or network mismatch".into(),
            )
            .into());
        }
        checkpoint.bitcoin_anchor = Some(anchor);
        checkpoint.sign(operator_key)?;
        let result = checkpoint.clone();
        self.save_checkpoints()?;
        Ok(result)
    }

    pub fn status(&self) -> Result<TransparencyStatus> {
        let leaves = self.leaf_hashes(self.events.len())?;
        let registered: HashSet<_> = self
            .events
            .iter()
            .filter(|e| e.action == NameAction::Register)
            .map(|e| &e.identifier_hash)
            .collect();
        Ok(TransparencyStatus {
            log_size: self.events.len() as u64,
            log_root: hex::encode(merkle_root(&leaves)),
            latest_checkpoint_hash: self
                .checkpoints
                .last()
                .map(TransparencyCheckpoint::checkpoint_hash)
                .transpose()?,
            registered_identifiers: registered.len() as u64,
            key_rotations: self
                .events
                .iter()
                .filter(|e| e.action == NameAction::RotateKey)
                .count() as u64,
            revocations: self
                .events
                .iter()
                .filter(|e| e.action == NameAction::Revoke)
                .count() as u64,
            map_root: None,
            consistency_status: "valid".into(),
        })
    }

    fn verify_replay(&self) -> Result<()> {
        let mut histories: HashMap<&str, Vec<NameEvent>> = HashMap::new();
        for event in &self.events {
            histories
                .entry(&event.identifier_hash)
                .or_default()
                .push(event.clone());
        }
        for history in histories.values() {
            verify_identifier_history(history)?;
        }
        Ok(())
    }

    fn save_checkpoints(&self) -> Result<()> {
        let path = self.dir.join(CHECKPOINTS_FILE);
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&self.checkpoints)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }
}
