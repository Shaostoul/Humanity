//! Merkle-tree selective disclosure (Phase 6b PR 2).
//!
//! Real, shippable selective disclosure for Verifiable Credentials. The issuer
//! commits to a vector of claim fields by Merkle-hashing them with BLAKE3 (PQ-
//! secure) and embedding the root in the VC. The holder later proves a specific
//! claim is in the original credential by presenting just that leaf's Merkle
//! authentication path — without revealing the rest of the credential.
//!
//! This is not the full zk-STARK power promised in the plan (no general
//! arithmetic predicates), but it's the right shape for the most common
//! selective-disclosure use cases:
//!
//!   - "I have a graduation_v1 credential from MIT" (reveal: degree + issuer)
//!     without revealing GPA, dates, etc.
//!   - "I am over 18" (reveal: age_bucket=adult) without revealing birthdate.
//!     Issuer pre-computes age_bucket leaf at credential issuance time.
//!   - "I hold credential X" (reveal: credential type) without leaking content.
//!
//! Full zk-STARK proofs (winterfell or Plonky2 with custom AIR circuits)
//! can be added later as a separate `stark_proof_v1` schema and verifier.
//! The wire format is `merkle_disclosure_v1` so it doesn't conflict.
//!
//! # Wire format
//!
//! - leaf:       BLAKE3(field_name || b"\0" || field_value_canonical_cbor)
//! - root:       BLAKE3 of the binary tree built bottom-up; odd levels promote
//! - path:       Vec<(sibling_hash: [u8; 32], is_left: bool)>
//! - verify:     fold path from leaf upward; final must equal root

use crate::relay::core::error::{Error, Result};

/// Hash size: BLAKE3 is 32 bytes.
pub const HASH_LEN: usize = 32;

/// One step in a Merkle authentication path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathStep {
    /// The sibling hash at this level.
    pub sibling: [u8; HASH_LEN],
    /// `true` if our position at this level is the LEFT child (sibling is right).
    /// `false` if we are the RIGHT child.
    pub is_left: bool,
}

/// Compute the leaf hash for a (field_name, field_value_canonical_cbor) pair.
/// The 0x00 separator prevents collisions between values containing the field name.
pub fn leaf_hash(field_name: &str, value_canonical_cbor: &[u8]) -> [u8; HASH_LEN] {
    let mut h = blake3::Hasher::new();
    h.update(field_name.as_bytes());
    h.update(&[0x00]);
    h.update(value_canonical_cbor);
    *h.finalize().as_bytes()
}

/// Combine two child hashes into a parent. Order is deterministic by left/right.
fn combine(left: &[u8; HASH_LEN], right: &[u8; HASH_LEN]) -> [u8; HASH_LEN] {
    let mut h = blake3::Hasher::new();
    h.update(left);
    h.update(right);
    *h.finalize().as_bytes()
}

/// Build a Merkle root over a set of pre-hashed leaves. Use this on the issuer
/// side when constructing a credential's commitment.
///
/// Returns the root and an in-order list of all internal node hashes (for
/// reference; clients building proofs should use [`build_path`]).
pub fn build_root(mut leaves: Vec<[u8; HASH_LEN]>) -> [u8; HASH_LEN] {
    if leaves.is_empty() {
        return *blake3::hash(b"").as_bytes();
    }
    while leaves.len() > 1 {
        let mut next = Vec::with_capacity((leaves.len() + 1) / 2);
        for chunk in leaves.chunks(2) {
            if chunk.len() == 2 {
                next.push(combine(&chunk[0], &chunk[1]));
            } else {
                // Promote unpaired leaf to next level (canonical Merkle convention)
                next.push(chunk[0]);
            }
        }
        leaves = next;
    }
    leaves[0]
}

/// Build the authentication path for `target_index` given all leaves.
pub fn build_path(
    mut leaves: Vec<[u8; HASH_LEN]>,
    mut target_index: usize,
) -> Result<Vec<PathStep>> {
    if leaves.is_empty() || target_index >= leaves.len() {
        return Err(Error::InvalidField {
            field: "target_index".into(),
            reason: "out of range".into(),
        });
    }

    let mut path = Vec::new();
    while leaves.len() > 1 {
        let mut next = Vec::with_capacity((leaves.len() + 1) / 2);
        for (chunk_idx, chunk) in leaves.chunks(2).enumerate() {
            if chunk.len() == 2 {
                if target_index / 2 == chunk_idx {
                    let is_left = target_index % 2 == 0;
                    let sibling = if is_left { chunk[1] } else { chunk[0] };
                    path.push(PathStep { sibling, is_left });
                }
                next.push(combine(&chunk[0], &chunk[1]));
            } else {
                next.push(chunk[0]);
                // No sibling at this level for the promoted leaf
            }
        }
        leaves = next;
        target_index /= 2;
    }
    Ok(path)
}

/// Verify a Merkle authentication path: starting from `leaf`, fold upward
/// through `path` and compare the result to `expected_root`.
pub fn verify_path(
    leaf: &[u8; HASH_LEN],
    path: &[PathStep],
    expected_root: &[u8; HASH_LEN],
) -> bool {
    let mut current = *leaf;
    for step in path {
        current = if step.is_left {
            combine(&current, &step.sibling)
        } else {
            combine(&step.sibling, &current)
        };
    }
    current == *expected_root
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> [u8; HASH_LEN] {
        leaf_hash(s, s.as_bytes())
    }

    #[test]
    fn single_leaf_root_is_the_leaf() {
        let leaf = h("only");
        let root = build_root(vec![leaf]);
        assert_eq!(root, leaf);
    }

    #[test]
    fn two_leaf_root_is_combined() {
        let l = h("left");
        let r = h("right");
        let root = build_root(vec![l, r]);
        assert_eq!(root, combine(&l, &r));
    }

    #[test]
    fn path_for_first_of_two_verifies() {
        let l = h("first");
        let r = h("second");
        let leaves = vec![l, r];
        let root = build_root(leaves.clone());
        let path = build_path(leaves, 0).unwrap();
        assert!(verify_path(&l, &path, &root));
    }

    #[test]
    fn path_for_second_of_two_verifies() {
        let l = h("first");
        let r = h("second");
        let leaves = vec![l, r];
        let root = build_root(leaves.clone());
        let path = build_path(leaves, 1).unwrap();
        assert!(verify_path(&r, &path, &root));
    }

    #[test]
    fn wrong_leaf_fails_verification() {
        let leaves = vec![h("a"), h("b"), h("c"), h("d")];
        let root = build_root(leaves.clone());
        let path = build_path(leaves.clone(), 2).unwrap();
        let attacker_leaf = h("attacker");
        assert!(!verify_path(&attacker_leaf, &path, &root));
    }

    #[test]
    fn tampered_path_fails_verification() {
        let leaves = vec![h("a"), h("b"), h("c"), h("d")];
        let root = build_root(leaves.clone());
        let mut path = build_path(leaves.clone(), 1).unwrap();
        // Flip a sibling bit
        path[0].sibling[0] ^= 0xFF;
        assert!(!verify_path(&leaves[1], &path, &root));
    }

    #[test]
    fn odd_count_promotes_correctly() {
        let leaves = vec![h("a"), h("b"), h("c")];
        let root = build_root(leaves.clone());
        for i in 0..3 {
            let path = build_path(leaves.clone(), i).unwrap();
            assert!(verify_path(&leaves[i], &path, &root), "leaf {} failed", i);
        }
    }

    #[test]
    fn five_leaves_all_verify() {
        let leaves: Vec<_> = (0..5)
            .map(|i| h(&format!("leaf_{}", i)))
            .collect();
        let root = build_root(leaves.clone());
        for i in 0..5 {
            let path = build_path(leaves.clone(), i).unwrap();
            assert!(verify_path(&leaves[i], &path, &root), "leaf {} failed", i);
        }
    }

    #[test]
    fn out_of_range_index_errors() {
        let leaves = vec![h("a"), h("b")];
        assert!(build_path(leaves, 5).is_err());
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let h1 = leaf_hash("name", b"alice");
        let h2 = leaf_hash("name", b"alice");
        assert_eq!(h1, h2);
    }

    #[test]
    fn leaf_hash_separator_prevents_collisions() {
        // "name" + "alice" should NOT collide with "namealice" + ""
        let with_field = leaf_hash("name", b"alice");
        let glued = leaf_hash("namealice", b"");
        assert_ne!(with_field, glued);
    }
}
