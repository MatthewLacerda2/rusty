//! How a reader learns a scene-level value changed with one compare, instead of comparing a
//! copy each frame.

use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Equal exactly when no write came between two reads, or one value is a clone of the
/// other; a new or loaded value takes a fresh one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision(u64);

impl Default for Revision {
    fn default() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// Every `&mut` access takes a fresh revision, even one that writes nothing: a spurious move only
/// makes the reader look closer, never miss a change. Saves and compares as the value alone.
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
