//! src/core/hulls.rs — the convex decompositions of concave meshes this process
//! knows, at unit scale, keyed by the mesh's content (#1940).
//!
//! A concave mesh collider on a dynamic body is split into convex pieces with
//! V-HACD (`physics::shape::decompose`, #1886), which takes up to a few hundred
//! milliseconds a mesh. The pieces are kept here, as plain data, so the physics
//! (a sim module, which never reads a file) finds them without running V-HACD:
//!
//! - **Loaded**: the asset layer reads a model's hull sidecar when it imports
//!   the model (`asset::hull_sidecar`) and [`keep`]s each entry whose mesh the model
//!   still has, so the first Play of a fresh launch decomposes nothing.
//! - **Fresh**: a decomposition the physics ran is kept with [`keep_fresh`], and a
//!   dev build writes the fresh ones into their models' sidecars ([`take_fresh`]).
//!
//! **Key.** A mesh's [`Key`] is the content hash of its positions and indices, as
//! the Mesh component holds them, and their counts: unscaled, since one
//! decomposition serves every scale (the body's scale is applied to the hulls).
//!
//! **Determinism.** V-HACD is single-threaded and seed-free, so a kept entry is the
//! same bits a fresh decomposition of the same mesh gives: whether a piece set came
//! from here or from a run never changes what the sim sees. Bounded at [`KEPT`]
//! entries; past it the oldest is dropped, and a later build decomposes it again.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// One convex piece, at unit scale: its points and the triangles V-HACD hulled them
/// with.
#[derive(Clone, Debug, PartialEq)]
pub struct Hull {
    pub points: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

/// A mesh's convex pieces. Empty: V-HACD found no piece with volume (a flat
/// mesh), which the caller keeps as a trimesh.
pub type Pieces = Arc<Vec<Hull>>;

/// What a decomposition is keyed by: the content hash of the mesh's positions and
/// indices, and their counts.
pub type Key = (u64, usize, usize);

/// How many decompositions are kept: every distinct concave prop of a
/// Dust2-sized level, with room.
pub const KEPT: usize = 1024;

#[derive(Default)]
struct Store {
    /// Insertion order, oldest first, for dropping past [`KEPT`].
    order: Vec<Key>,
    pieces: BTreeMap<Key, Pieces>,
    /// Decompositions run this session and not yet written to a sidecar.
    fresh: BTreeMap<Key, Pieces>,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut Store) -> T) -> T {
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Store::default))
}

/// The key of the mesh `positions` and `indices` (unscaled).
pub fn key(positions: &[[f32; 3]], indices: &[u32]) -> Key {
    let mut bytes = Vec::with_capacity(positions.len() * 12 + indices.len() * 4);
    for p in positions {
        bytes.extend(p.iter().flat_map(|c| c.to_bits().to_le_bytes()));
    }
    for i in indices {
        bytes.extend(i.to_le_bytes());
    }
    let hash = crate::core::hash::content(&bytes);
    (hash, positions.len(), indices.len())
}

/// The kept pieces of the mesh `key`, if any.
pub fn get(key: Key) -> Option<Pieces> {
    with(|s| s.pieces.get(&key).cloned())
}

/// Keep `pieces` for `key` (read from a sidecar, or a failed run not worth
/// writing down).
pub fn keep(key: Key, pieces: Pieces) {
    with(|s| s.insert(key, pieces));
}

/// Keep `pieces` a decomposition just made for `key`, and mark them for writing.
pub fn keep_fresh(key: Key, pieces: Pieces) {
    with(|s| {
        s.fresh.insert(key, pieces.clone());
        s.insert(key, pieces);
    });
}

/// The decompositions made since the last call, by key, for a dev build to write.
pub fn take_fresh() -> Vec<(Key, Pieces)> {
    with(|s| std::mem::take(&mut s.fresh).into_iter().collect())
}

/// Forget `key`, as a fresh launch has (tests).
#[cfg(test)]
pub fn forget(key: Key) {
    with(|s| {
        s.pieces.remove(&key);
        s.fresh.remove(&key);
        s.order.retain(|k| *k != key);
    });
}

impl Store {
    fn insert(&mut self, key: Key, pieces: Pieces) {
        if self.pieces.insert(key, pieces).is_none() {
            self.order.push(key);
        }
        if self.order.len() > KEPT {
            let oldest = self.order.remove(0);
            self.pieces.remove(&oldest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(x: f32) -> Pieces {
        let points = vec![[x, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]];
        Arc::new(vec![Hull {
            points,
            triangles: vec![[0, 1, 2]],
        }])
    }

    #[test]
    fn a_key_follows_every_coordinate_and_index() {
        let p = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let base = key(&p, &[0, 1, 2]);
        assert_eq!(base, key(&p, &[0, 1, 2]));
        assert_ne!(base, key(&p, &[0, 2, 1]));
        let mut q = p;
        q[2][1] = 1.0 + f32::EPSILON;
        assert_ne!(base, key(&q, &[0, 1, 2]));
        assert_eq!((base.1, base.2), (3, 3));
    }

    #[test]
    fn fresh_pieces_are_kept_and_handed_out_once() {
        let k = (0x1940_0001, 4, 3);
        keep_fresh(k, one(1.0));
        assert_eq!(get(k), Some(one(1.0)));
        let fresh = take_fresh();
        assert!(fresh.iter().any(|(f, _)| *f == k));
        assert!(!take_fresh().iter().any(|(f, _)| *f == k));
        assert_eq!(get(k), Some(one(1.0)), "still kept after the write");
        let loaded = (0x1940_0002, 4, 3);
        keep(loaded, one(2.0));
        assert!(!take_fresh().iter().any(|(f, _)| *f == loaded));
    }

    #[test]
    fn past_the_bound_the_oldest_goes() {
        let mut store = Store::default();
        for n in 0..=KEPT as u64 {
            store.insert((n, 0, 0), one(1.0));
        }
        assert!(!store.pieces.contains_key(&(0, 0, 0)));
        assert!(store.pieces.contains_key(&(KEPT as u64, 0, 0)));
        assert_eq!(store.pieces.len(), KEPT);
    }
}
