use anchor_lang::prelude::*;
use sha2::{Sha256, Digest};

pub fn compute_merkle_leaf(user: &Pubkey, score: u32) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(user.as_ref());
    hasher.update(&score.to_le_bytes());
    hasher.finalize().into()
}

fn hash_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    if a <= b {
        hasher.update(a);
        hasher.update(b);
    } else {
        hasher.update(b);
        hasher.update(a);
    }
    hasher.finalize().into()
}

/// Verify Merkle proof
pub fn verify_merkle_proof(
    root: &[u8; 32],
    leaf: &[u8; 32],
    proof: &[[u8; 32]],
) -> Result<bool> {
    let mut current = *leaf;
    
    for sibling in proof {
        current = hash_pair(&current, sibling);
    }
    
    Ok(current == *root)
}