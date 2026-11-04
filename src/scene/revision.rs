//! src/scene/revision.rs — how a reader learns a scene-level value changed without
//! comparing it (#1706).
//!
//! The renderer keeps objects derived from the material library, the lightmap set
//! and the probe field, and must learn when one of them changes. Comparing a copy
//! each frame costs their size (a few hundred µs for a Kolvik-sized library and
//! lightmap set in a dev build); a [`Revision`] costs one compare. A value moves
//! its revision at every write: [`Revisioned`] at every `&mut` access, as Bevy's
//! `Mut<T>` marks a component changed on `DerefMut`, and `ProbeVolume` in its own
//! write methods. A spurious move (a `&mut` taken and nothing written) only makes
//! the reader look closer, never miss a change.
//!
//! The counter is a plain atomic, no clock and no RNG, as [`super::SceneId`]'s is:
//! a revision is only ever compared for equality, never read as sim, saved or
//! digested.

use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

static NEXT: AtomicU64 = AtomicU64::new(1);

/// What a value was last written as. Two reads give the same revision exactly when
/// no write came between them, or one value is a copy of the other: a new or
/// loaded value takes a fresh one (`Default`), and a clone keeps its original's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision(u64);

impl Default for Revision {
    fn default() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// A value whose every `&mut` access takes a fresh [`Revision`]. Reads go through
/// `Deref`; it saves, loads and compares as the value alone.
#[derive(Clone, Debug, Default)]
pub struct Revisioned<T> {
    value: T,
    revision: Revision,
}

impl<T> Revisioned<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            revision: Revision::default(),
        }
    }

    /// What the value was last written as.
    pub fn revision(&self) -> Revision {
        self.revision
    }
}

impl<T> From<T> for Revisioned<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Deref for Revisioned<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for Revisioned<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.revision = Revision::default();
        &mut self.value
    }
}

impl<T: PartialEq> PartialEq for Revisioned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: Serialize> Serialize for Revisioned<T> {
    fn serialize<S: Serializer>(&self, out: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(out)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Revisioned<T> {
    fn deserialize<D: Deserializer<'de>>(input: D) -> Result<Self, D::Error> {
        T::deserialize(input).map(Self::new)
    }
}

#[cfg(test)]
#[path = "revision_tests.rs"]
mod tests;
