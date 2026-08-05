use crate::crypto::{verify_message_signature, verify_signed_profile};
use crate::{Result, SignedPaymentProfile};

use super::event::profile_hash;
use super::tree::{merkle_root, node_hash};
use super::{
    MerkleConsistencyProof, MerkleInclusionProof, NameAction, NameEvent, PinnedCheckpoint,
    TransparencyCheckpoint, TransparencyError,
};

fn decode_hash(value: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(value).map_err(|_| TransparencyError::InvalidInclusionProof)?;
    bytes
        .try_into()
        .map_err(|_| TransparencyError::InvalidInclusionProof.into())
}

pub fn verify_inclusion_proof(proof: &MerkleInclusionProof) -> Result<bool> {
    if proof.tree_size == 0 || proof.leaf_index >= proof.tree_size {
        return Err(TransparencyError::InvalidInclusionProof.into());
    }
    let mut hash = decode_hash(&proof.leaf_hash)?;
    let mut index = proof.leaf_index;
    let mut last = proof.tree_size - 1;
    for sibling_hex in &proof.audit_path {
        let sibling = decode_hash(sibling_hex)?;
        if index % 2 == 1 || index == last {
            hash = node_hash(&sibling, &hash);
            while index.is_multiple_of(2) && index != 0 {
                index /= 2;
                last /= 2;
            }
        } else {
            hash = node_hash(&hash, &sibling);
        }
        index /= 2;
        last /= 2;
    }
    Ok(hex::encode(hash) == proof.root_hash && last == 0)
}

pub fn verify_consistency_proof(proof: &MerkleConsistencyProof) -> Result<bool> {
    if proof.old_tree_size == 0
        || proof.old_tree_size > proof.new_tree_size
        || proof.proof.len() != proof.new_tree_size as usize
    {
        return Err(TransparencyError::InvalidConsistencyProof.into());
    }
    let leaves: Vec<[u8; 32]> = proof
        .proof
        .iter()
        .map(|h| decode_hash(h))
        .collect::<Result<_>>()?;
    Ok(
        hex::encode(merkle_root(&leaves[..proof.old_tree_size as usize])) == proof.old_root
            && hex::encode(merkle_root(&leaves)) == proof.new_root,
    )
}

pub fn verify_checkpoint(checkpoint: &TransparencyCheckpoint) -> Result<bool> {
    if checkpoint.version != 1 || checkpoint.log_size == 0 {
        return Ok(false);
    }
    verify_message_signature(
        &checkpoint.signing_message()?,
        &checkpoint.operator_signature,
        &checkpoint.operator_pubkey,
    )
}

pub fn verify_checkpoint_transition(
    pinned: &PinnedCheckpoint,
    current: &TransparencyCheckpoint,
    consistency: Option<&MerkleConsistencyProof>,
) -> Result<()> {
    if !verify_checkpoint(current)? || current.operator_pubkey != pinned.operator_pubkey {
        return Err(TransparencyError::InvalidCheckpointSignature.into());
    }
    if current.log_size < pinned.tree_size {
        return Err(TransparencyError::CheckpointRollback.into());
    }
    if current.log_size == pinned.tree_size {
        if current.log_root != pinned.root_hash {
            return Err(TransparencyError::ConflictingCheckpoint.into());
        }
        return Ok(());
    }
    let proof = consistency.ok_or(TransparencyError::InvalidConsistencyProof)?;
    if proof.old_tree_size != pinned.tree_size
        || proof.new_tree_size != current.log_size
        || proof.old_root != pinned.root_hash
        || proof.new_root != current.log_root
        || !verify_consistency_proof(proof)?
    {
        return Err(TransparencyError::InvalidConsistencyProof.into());
    }
    Ok(())
}

pub fn verify_identifier_history(events: &[NameEvent]) -> Result<()> {
    if events.is_empty() || events[0].action != NameAction::Register || events[0].sequence != 0 {
        return Err(TransparencyError::BrokenIdentifierHistory(
            "history must start at registration sequence 0".into(),
        )
        .into());
    }
    let identifier = &events[0].identifier_hash;
    let mut authorized_key = events[0].identity_pubkey.clone();
    let mut previous_hash: Option<String> = None;
    let mut revoked = false;
    for (index, event) in events.iter().enumerate() {
        if &event.identifier_hash != identifier || event.sequence != index as u64 {
            return Err(TransparencyError::BrokenIdentifierHistory(
                "identifier substitution or sequence rollback".into(),
            )
            .into());
        }
        if event.previous_event_hash != previous_hash {
            return Err(TransparencyError::BrokenIdentifierHistory(
                "previous event hash mismatch".into(),
            )
            .into());
        }
        if revoked {
            return Err(TransparencyError::IdentifierRevoked.into());
        }
        if event.action == NameAction::RecoverKey {
            return Err(TransparencyError::RecoveryDisabled.into());
        }
        let signing_key = if event.action == NameAction::RotateKey {
            let rotation = event
                .rotation
                .as_ref()
                .ok_or_else(|| TransparencyError::InvalidRotation("missing dual proof".into()))?;
            if rotation.previous_pubkey != authorized_key
                || rotation.new_pubkey != event.identity_pubkey
                || rotation.identifier_hash != event.identifier_hash
                || rotation.previous_event_hash
                    != event.previous_event_hash.clone().unwrap_or_default()
                || rotation.sequence != event.sequence
                || !rotation.verify()?
            {
                return Err(TransparencyError::InvalidRotation(
                    "old authorization or new acceptance failed".into(),
                )
                .into());
            }
            authorized_key.clone()
        } else {
            if event.identity_pubkey != authorized_key {
                return Err(TransparencyError::UnauthorizedKeyReplacement.into());
            }
            authorized_key.clone()
        };
        if !verify_message_signature(
            &event.signing_message()?,
            &event.owner_signature,
            &signing_key,
        )? {
            return Err(TransparencyError::InvalidEventSignature.into());
        }
        if event.action == NameAction::RotateKey {
            authorized_key = event.identity_pubkey.clone();
        }
        revoked = event.action == NameAction::Revoke;
        previous_hash = Some(event.event_hash()?);
    }
    Ok(())
}

pub fn verify_key_continuity(events: &[NameEvent]) -> Result<bool> {
    verify_identifier_history(events)?;
    Ok(true)
}

pub fn verify_event_profile(event: &NameEvent, profile: &SignedPaymentProfile) -> Result<bool> {
    if event.profile_hash != profile_hash(profile)?
        || event.identity_pubkey != profile.profile.identity_pubkey
    {
        return Err(TransparencyError::ProfileHashMismatch.into());
    }
    verify_signed_profile(profile)
}
