//! `std`'s `HashMap` seeds from OS entropy, so its iteration order varies per process.
//! Where the walk order *is* behaviour, use a `BTreeMap` or sort. Build with
//! `Map::default()`: `new()` exists only for the random hasher.

pub use rustc_hash::{FxHashMap as Map, FxHashSet as Set};
