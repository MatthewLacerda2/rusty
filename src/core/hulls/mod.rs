//! src/core/hulls/mod.rs — the convex decompositions of concave meshes this process
//! knows, at unit scale, keyed by the mesh's content (#1940).
//!
//! A concave mesh collider on a dynamic body is split into convex pieces with
//! V-HACD (`physics::shape::decompose`, #1886), which takes up to a few hundred
//! milliseconds a mesh. The pieces are kept here, as plain data, so the physics
//! (a sim module, which never reads a file) finds them without running V-HACD:
//!
//! - **Loaded**: the asset layer reads a model's hull sidecar when it imports
//!   the model (`asset::import_cache::hulls`) and [`keep`]s each entry whose mesh
//!   the model still has, so the first Play of a fresh launch decomposes nothing.
//! - **Fresh**: a decomposition the physics ran is kept with [`keep_fresh`]. An
//!   authoring build ([`WRITES`]: the editor or `dev`) also marks it, and writes
//!   the marked ones into their models' sidecars (`take_fresh`); the player keeps
//!   no marks, so nothing piles up where nothing writes.
//!
//! **Key.** A mesh's [`Key`] is the content hash of its positions and indices, as
//! the Mesh component holds them, and their counts: unscaled, since one
//! decomposition serves every scale (the body's scale is applied to the hulls).
//!
//! **Determinism.** V-HACD is single-threaded and seed-free, so a kept entry is the
//! same bits a fresh decomposition of the same mesh gives, as long as the
//! decomposer is the same one: a sidecar carries [`DECOMPOSER`], and one made by
//! another is refused. Bounded at [`KEPT`] entries; past it the oldest is dropped,
//! and a later build decomposes it again.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Whether this build writes hull sidecars: an authoring build (the editor or
/// `dev`), never the shipped player (`--no-default-features`).
pub const WRITES: bool = cfg!(any(feature = "editor", feature = "dev"));

/// The parry release whose V-HACD made the pieces. A test holds it to
/// `Cargo.lock` (`physics::shape::decompose::fingerprint_tests`).
pub const PARRY: &str = "0.31.1";

/// What made a decomposition, written in every sidecar's header: bumped (the
/// leading number) whenever `physics::shape::decompose` changes its V-HACD
/// parameters or the filter that keeps a piece, and following [`PARRY`]. A sidecar
/// with another fingerprint is refused, so every machine simulates the hulls this
/// build would make. A test pins the parameters and the filter's slack.
pub const DECOMPOSER: &str = "1 vhacd-default has-volume-1e-5 parry3d-0.31.1";

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
    #[cfg(any(feature = "editor", feature = "dev"))]
    fresh: BTreeMap<Key, Pieces>,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut Store) -> T) -> T {
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Store::default))
}

/// The key of the mesh with unscaled `positions` and `indices`.
pub fn key(positions: impl ExactSizeIterator<Item = [f32; 3]>, indices: &[u32]) -> Key {
    let count = positions.len();
    let mut bytes = Vec::with_capacity(count * 12 + indices.len() * 4);
    for p in positions {
        bytes.extend(p.iter().flat_map(|c| c.to_bits().to_le_bytes()));
    }
    for i in indices {
        bytes.extend(i.to_le_bytes());
    }
    let hash = crate::core::hash::content(&bytes);
    (hash, count, indices.len())
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

/// Keep `pieces` a decomposition just made for `key`, and, in an authoring build,
/// mark them for writing.
pub fn keep_fresh(key: Key, pieces: Pieces) {
    with(|s| s.keep_fresh(key, pieces));
}

/// The decompositions made since the last call, by key, for an authoring build to
/// write.
#[cfg(any(feature = "editor", feature = "dev"))]
pub fn take_fresh() -> Vec<(Key, Pieces)> {
    with(Store::take_fresh)
}

/// Forget `key`, as a fresh launch has (tests).
#[cfg(test)]
pub fn forget(key: Key) {
    with(|s| {
        s.pieces.remove(&key);
        #[cfg(any(feature = "editor", feature = "dev"))]
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

    fn keep_fresh(&mut self, key: Key, pieces: Pieces) {
        #[cfg(any(feature = "editor", feature = "dev"))]
        self.fresh.insert(key, pieces.clone());
        self.insert(key, pieces);
    }

    #[cfg(any(feature = "editor", feature = "dev"))]
    fn take_fresh(&mut self) -> Vec<(Key, Pieces)> {
        std::mem::take(&mut self.fresh).into_iter().collect()
    }
}

#[cfg(test)]
mod tests;
