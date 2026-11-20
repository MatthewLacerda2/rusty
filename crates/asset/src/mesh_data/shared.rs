//! crates/asset/src/mesh_data/shared.rs — geometry held once and shared.
//!
//! A [`Shared`] buffer is one allocation that the import cache's sub-mesh and every
//! mesh instancing it point at: cloning one clones a pointer. It reads as a slice;
//! [`Shared::edit`] is its one way in for writing, and copies the buffer first when
//! anyone else still holds it, so an edit never reaches a sibling instance or the
//! cache. Replacing a buffer whole (`= vec.into()`) copies nothing.
//!
//! [`copies`] is a test hook (crates/CLAUDE.md): it counts the buffers an edit
//! really copied on this thread, for the tests that hold a load, a Play → Stop or an
//! Undo to none, compiled in every build since those tests are other crates', and
//! counting only on a thread that asked once.

use std::cell::Cell;
use std::sync::Arc;

/// A buffer shared by everything that holds it, copied on its first [`edit`](Self::edit)
/// by one holder of several.
#[derive(Debug, PartialEq)]
pub struct Shared<T>(Arc<Vec<T>>);

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COPIES: Cell<usize> = const { Cell::new(0) };
}

/// Buffers this thread copied to edit them (an [`edit`](Shared::edit) of a buffer
/// someone else still held) since the first call of this function on it, which
/// starts the count.
#[doc(hidden)]
pub fn copies() -> usize {
    ARMED.set(true);
    COPIES.get()
}

impl<T: Clone> Shared<T> {
    /// This buffer, writable: the same allocation when nothing else holds it, else a
    /// copy of it that this holder alone now points at (`Arc::make_mut`).
    pub fn edit(&mut self) -> &mut Vec<T> {
        if Arc::strong_count(&self.0) > 1 && ARMED.get() {
            COPIES.set(COPIES.get() + 1);
        }
        Arc::make_mut(&mut self.0)
    }
}

impl<T> Shared<T> {
    /// Whether `a` and `b` are one allocation: what sharing a buffer means.
    pub fn same(a: &Self, b: &Self) -> bool {
        Arc::ptr_eq(&a.0, &b.0)
    }

    /// Whether nothing else holds this buffer, so freeing it frees its memory.
    pub fn is_unique(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> Default for Shared<T> {
    fn default() -> Self {
        Self(Arc::new(Vec::new()))
    }
}

impl<T> std::ops::Deref for Shared<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<T> From<Vec<T>> for Shared<T> {
    fn from(buffer: Vec<T>) -> Self {
        Self(Arc::new(buffer))
    }
}

impl<T> FromIterator<T> for Shared<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        Vec::from_iter(items).into()
    }
}

impl<'a, T> IntoIterator for &'a Shared<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: PartialEq> PartialEq<Vec<T>> for Shared<T> {
    fn eq(&self, other: &Vec<T>) -> bool {
        *self.0 == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_copies_only_a_buffer_someone_else_holds() {
        let start = copies();
        let mut own: Shared<u32> = vec![1, 2].into();
        own.edit().push(3);
        assert_eq!(copies() - start, 0, "a buffer held once is edited in place");
        let held = own.clone();
        assert!(Shared::same(&own, &held));
        own.edit()[0] = 9;
        assert_eq!((&held[..], &own[..]), (&[1, 2, 3][..], &[9, 2, 3][..]));
        assert!(!Shared::same(&own, &held) && own.is_unique());
        assert_eq!(copies() - start, 1);
    }
}
