//! The sim's hash maps and sets (#764).
//!
//! `std`'s `HashMap` seeds SipHash from OS entropy per process, so the order it
//! iterates in changes from run to run: an unseeded RNG that never shows up as a
//! call. The sim is a pure function of (seed, inputs, fixed dt), so its maps hash
//! with FxHash instead, which has no seed. `clippy.toml` bans the `std` types
//! outside the platform modules, so this is the one place the choice is made.
//!
//! A deterministic order is still an arbitrary one: where the order a map is walked
//! in *is* behaviour (what steps, spawns, emits or serialises first), use a
//! `BTreeMap` or sort. Build with `Map::default()`: `new()` exists only for `std`'s
//! random hasher.

pub use rustc_hash::{FxHashMap as Map, FxHashSet as Set};
