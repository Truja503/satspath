use sha2::{Digest, Sha256};

use super::{MerkleInclusionProof, TransparencyError};

pub fn leaf_hash(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([0x00]);
    h.update(data);
    h.finalize().into()
}

pub fn node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([0x01]);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

pub fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    match leaves.len() {
        0 => Sha256::digest([]).into(),
        1 => leaves[0],
        n => {
            let split = largest_power_of_two_less_than(n);
            node_hash(
                &merkle_root(&leaves[..split]),
                &merkle_root(&leaves[split..]),
            )
        }
    }
}

pub fn inclusion_proof(
    leaves: &[[u8; 32]],
    index: usize,
) -> Result<MerkleInclusionProof, TransparencyError> {
    if index >= leaves.len() {
        return Err(TransparencyError::InvalidInclusionProof);
    }
    let mut path = Vec::new();
    inclusion_path(leaves, index, &mut path);
    Ok(MerkleInclusionProof {
        leaf_index: index as u64,
        tree_size: leaves.len() as u64,
        leaf_hash: hex::encode(leaves[index]),
        audit_path: path.into_iter().map(hex::encode).collect(),
        root_hash: hex::encode(merkle_root(leaves)),
    })
}

fn inclusion_path(leaves: &[[u8; 32]], index: usize, out: &mut Vec<[u8; 32]>) {
    if leaves.len() <= 1 {
        return;
    }
    let split = largest_power_of_two_less_than(leaves.len());
    if index < split {
        inclusion_path(&leaves[..split], index, out);
        out.push(merkle_root(&leaves[split..]));
    } else {
        inclusion_path(&leaves[split..], index - split, out);
        out.push(merkle_root(&leaves[..split]));
    }
}

fn largest_power_of_two_less_than(n: usize) -> usize {
    debug_assert!(n > 1);
    let p = 1usize << (usize::BITS - 1 - (n - 1).leading_zeros());
    if p == n {
        p / 2
    } else {
        p
    }
}
