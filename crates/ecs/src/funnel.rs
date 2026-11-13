//! crates/ecs/src/funnel.rs — the column write funnel: every raw mutable hecs access to
//! one column of one entity (#1705).
//!
//! Each write calls `World::touch` (`changes/stamp.rs`), which notes it for the
//! edit journal and stamps the change log, and only once the entity is found to
//! hold the column: a write to a column it lacks changed nothing, so it names
//! nothing (physics borrowing `rigidbody_mut` for a static collider with no
//! Rigidbody, the despawn walk over every reference column). Whole-entity writes
//! are `placement.rs`'s. The `ecs-write` lint flags a raw write anywhere else.

use super::changes::Channel;
use super::world::{RefMut, World};

impl World {
    /// Whether `id` is alive and carries column `T`.
    fn holds<T: hecs::Component>(&self, id: u32) -> bool {
        self.handles
            .get(id)
            .is_some_and(|h| self.inner.entity(h).is_ok_and(|e| e.has::<T>()))
    }

    /// Exclusive borrow of one component column, by stable id.
    pub(crate) fn component_mut<T: hecs::Component + Channel>(
        &mut self,
        id: u32,
    ) -> Option<RefMut<'_, T>> {
        if !self.holds::<T>(id) {
            return None;
        }
        self.touch::<T>(id);
        let handle = self.handles.get(id)?;
        self.inner.get::<&mut T>(handle).ok() // lint: allow(ecs-write) the funnel: touched above
    }

    /// Exclusive borrow of a column for state derived from the others (a collider's
    /// world box): no journal note and no change stamp, since nothing was edited.
    /// Never for a value a consumer or an Undo must see.
    pub(crate) fn derived_mut<T: hecs::Component>(&mut self, id: u32) -> Option<RefMut<'_, T>> {
        let handle = self.handles.get(id)?;
        // lint: allow(ecs-write) derived state, rebuilt from stamped columns: nothing to report
        self.inner.get::<&mut T>(handle).ok()
    }

    /// Attach (`Some`) or detach (`None`) a component column. Returns `false`
    /// when the entity does not exist. Detaching a column it lacks stamps nothing.
    pub(crate) fn set_component<T: hecs::Component + Channel>(
        &mut self,
        id: u32,
        value: Option<T>,
    ) -> bool {
        let Some(handle) = self.handles.get(id) else {
            return false;
        };
        match value {
            Some(v) => {
                self.touch::<T>(id);
                let _ = self.inner.insert_one(handle, v); // lint: allow(ecs-write) the funnel: touched above
            }
            None => {
                self.take_component::<T>(id);
            }
        }
        true
    }

    /// Detach and return a component column (`None` when absent or the entity
    /// is dead, and then nothing is stamped).
    pub(crate) fn take_component<T: hecs::Component + Channel>(&mut self, id: u32) -> Option<T> {
        if !self.holds::<T>(id) {
            return None;
        }
        self.touch::<T>(id);
        let handle = self.handles.get(id)?;
        self.inner.remove_one::<T>(handle).ok() // lint: allow(ecs-write) the funnel: touched above
    }

    /// Borrow column `A` mutably and a DIFFERENT column `B` for reading, of the same
    /// entity at once — sound because they're independent hecs columns. `None` when
    /// the entity is missing or lacks the mandatory `A`; only `A` is stamped.
    pub(crate) fn with_mut_and_ref<A, B, R>(
        &mut self,
        id: u32,
        f: impl FnOnce(&mut A, Option<&B>) -> R,
    ) -> Option<R>
    where
        A: hecs::Component + Channel,
        B: hecs::Component,
    {
        if !self.holds::<A>(id) {
            return None;
        }
        self.touch::<A>(id);
        let handle = self.handles.get(id)?;
        let mut query = self
            .inner
            // lint: allow(ecs-write) the funnel: touched above
            .query_one::<(&mut A, Option<&B>)>(handle)
            .ok()?;
        let (a, b) = query.get()?;
        Some(f(a, b))
    }
}
