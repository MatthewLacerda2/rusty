//! Convex decompositions of concave meshes (V-HACD takes up to a few hundred ms a mesh),
//! kept as plain data so the sim finds them without running V-HACD or reading a file.
//! A mesh's [`Key`] hashes its unscaled positions and indices (one decomposition serves
//! every scale). V-HACD is single-threaded and seed-free, so a sidecar from another
//! [`DECOMPOSER`] is refused. Bounded at [`KEPT`]; the oldest is dropped past it.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub const WRITES: bool = cfg!(any(feature = "editor", feature = "dev"));

pub const PARRY: &str = "0.31.1";

pub const DECOMPOSER: &str = "1 vhacd-default has-volume-1e-5 parry3d-0.31.1";

#[derive(Clone, Debug, PartialEq)]
pub struct Hull {
    pub points: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

pub type Pieces = Arc<Vec<Hull>>;

pub type Key = (u64, usize, usize);

pub const KEPT: usize = 1024;

#[derive(Default)]
struct Store {
    order: Vec<Key>,
    pieces: BTreeMap<Key, Pieces>,
    #[cfg(any(feature = "editor", feature = "dev"))]
    fresh: BTreeMap<Key, Pieces>,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut Store) -> T) -> T {
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Store::default))
}

pub fn key(positions: impl ExactSizeIterator<Item = [f32; 3]>, indices: &[u32]) -> Key {
    let count = positions.len();
    let mut bytes = Vec::with_capacity(count * 12 + indices.len() * 4);
    for p in positions {
        bytes.extend(p.iter().flat_map(|c| c.to_bits().to_le_bytes()));
    }
    for i in indices {
        bytes.extend(i.to_le_bytes());
    }
    let hash = crate::hash::content(&bytes);
    (hash, count, indices.len())
}

pub fn get(key: Key) -> Option<Pieces> {
    with(|s| s.pieces.get(&key).cloned())
}

/// Keeps without marking for a sidecar: pieces read from one, or a failed run's none.
pub fn keep(key: Key, pieces: Pieces) {
    with(|s| s.insert(key, pieces));
}

pub fn keep_fresh(key: Key, pieces: Pieces) {
    with(|s| s.keep_fresh(key, pieces));
}

#[cfg(any(feature = "editor", feature = "dev"))]
pub fn take_fresh() -> Vec<(Key, Pieces)> {
    with(Store::take_fresh)
}

/// A test hook: forgets `key`, as a fresh launch has.
#[doc(hidden)]
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
