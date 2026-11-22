//! A raw mutable access here touches only once the entity holds the column: a write to a
//! column it lacks changed nothing, so it names nothing.

use super::changes::Channel;
use super::world::{RefMut, World};

impl World {
    fn holds<T: hecs::Component>(&self, id: u32) -> bool {
        self.handles
            .get(id)
            .is_some_and(|h| self.inner.entity(h).is_ok_and(|e| e.has::<T>()))
    }

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

    /// For state derived from the other columns (a collider's world box): no journal note, no
    /// stamp. Never for a value a consumer or an Undo must see.
    pub(crate) fn derived_mut<T: hecs::Component>(&mut self, id: u32) -> Option<RefMut<'_, T>> {
        let handle = self.handles.get(id)?;
        // lint: allow(ecs-write) derived state, rebuilt from stamped columns: nothing to report
        self.inner.get::<&mut T>(handle).ok()
    }

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
                super::walks::moved_if(|| !self.holds::<T>(id));
                self.touch::<T>(id);
                let _ = self.inner.insert_one(handle, v); // lint: allow(ecs-write) the funnel: touched above
            }
            None => {
                self.take_component::<T>(id);
            }
        }
        true
    }

    pub(crate) fn take_component<T: hecs::Component + Channel>(&mut self, id: u32) -> Option<T> {
        if !self.holds::<T>(id) {
            return None;
        }
        self.touch::<T>(id);
        super::walks::moved_if(|| true);
        let handle = self.handles.get(id)?;
        self.inner.remove_one::<T>(handle).ok() // lint: allow(ecs-write) the funnel: touched above
    }

    /// `A` borrowed mutably beside a different column `B` read; only `A` is stamped.
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
