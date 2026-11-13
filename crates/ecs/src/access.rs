//! crates/ecs/src/access.rs — typed component access on the World (#344, #345).
//!
//! Stage 2 of the megastruct split (#345): every optional first-class
//! component is now its own hecs column, so these accessors are thin
//! passthroughs to `World`'s generic `component`/`component_mut`/
//! `has_component`/`set_component`/`take_component` helpers — no projection
//! needed. `CompRef`/`CompMut` are thin newtype wrappers around hecs's own
//! borrow guards (see the `Clone` note below for why they're not aliases).
//!
//! Shape rules the seam relies on:
//! - `CompRef`/`CompMut` deliberately do NOT implement `Clone` themselves —
//!   `hecs::Ref`/`RefMut` do (a cheap borrow-guard clone), which would make
//!   `guard.clone()` at a call site silently resolve to a re-borrow instead of
//!   `T::clone()` via `Deref`. Wrapping suppresses that trap so `.clone()`
//!   keeps meaning "clone the component", matching pre-#345 behaviour.
//! - Ordered iteration (`ids_with_*`) yields insertion order — the determinism
//!   contract (physics pair ordering, replay byte-identity). Since #346 each is
//!   a narrow hecs query (archetype-filtered, then sorted by `Core::seq`), so
//!   its cost tracks the carrier population, not the scene size.

use std::ops::{Deref, DerefMut};

use rusty_components::registry::first_class;
use rusty_components::{ImageComponent, MeshComponent, PrefabLink};

use super::world::{Ref, RefMut, World};

/// Shared borrow of one component column.
pub struct CompRef<'w, T: ?Sized>(Ref<'w, T>);

impl<'w, T: ?Sized> CompRef<'w, T> {
    pub(super) fn new(guard: Ref<'w, T>) -> Self {
        Self(guard)
    }
}

impl<T: ?Sized> Deref for CompRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

/// Exclusive borrow of one component column.
pub struct CompMut<'w, T: ?Sized>(RefMut<'w, T>);

impl<'w, T: ?Sized> CompMut<'w, T> {
    pub(super) fn new(guard: RefMut<'w, T>) -> Self {
        Self(guard)
    }
}

impl<T: ?Sized> Deref for CompMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> DerefMut for CompMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

/// Generate the accessor family for one OPTIONAL first-class component:
/// `get` / `get_mut` (None when absent), `has`, `set` (Some = attach/replace,
/// None = detach), `take` (detach returning the component — the sim's
/// take/simulate/write-back idiom), and `ids_with` (stable ids carrying the
/// component, in insertion order).
///
/// A uniform family on purpose: `make dead-pub` lists the members nothing calls
/// today (most `take_*`), and they stay so every component has the same shape.
macro_rules! optional_component_accessors {
    ($($ty:ty => $get:ident, $get_mut:ident, $has:ident, $set:ident, $take:ident, $ids_with:ident;)*) => {
        impl World {
            $(
                pub fn $get(&self, id: u32) -> Option<CompRef<'_, $ty>> {
                    self.component::<$ty>(id).map(CompRef::new)
                }

                pub fn $get_mut(&mut self, id: u32) -> Option<CompMut<'_, $ty>> {
                    self.component_mut::<$ty>(id).map(CompMut::new)
                }

                pub fn $has(&self, id: u32) -> bool {
                    self.has_component::<$ty>(id)
                }

                /// Attach (`Some`) or detach (`None`) the component. Returns
                /// `false` when the entity does not exist.
                pub fn $set(&mut self, id: u32, value: Option<$ty>) -> bool {
                    self.set_component::<$ty>(id, value)
                }

                /// Detach and return the component (`None` when absent or the
                /// entity is dead).
                pub fn $take(&mut self, id: u32) -> Option<$ty> {
                    self.take_component::<$ty>(id)
                }

                /// Stable ids of the entities carrying this component, in
                /// insertion order (the determinism-safe iteration order).
                /// A narrow hecs query (#346): cost tracks the carrier
                /// population, not the scene size.
                pub fn $ids_with(&self) -> Vec<u32> {
                    self.ids_with_component::<$ty>()
                }
            )*
        }
    };
}

/// One registry row's accessor family. The Collider writes its own
/// (`world_space::collider`, which keeps its AABB set) but for `take`.
macro_rules! row_accessors {
    (collider: $ty:ty, $get_mut:ident $has:ident $set:ident $take:ident $ids_with:ident) => {
        impl World {
            /// Detach and return the collider (`None` when absent or the entity is
            /// dead).
            pub fn $take(&mut self, id: u32) -> Option<$ty> {
                self.take_component::<$ty>(id)
            }
        }
    };
    ($get:ident: $ty:ty, $($acc:ident)*) => {
        optional_component_accessors! { $ty => $get, $($acc),*; }
    };
}

/// Every registry row's accessor family (`components::registry`).
macro_rules! registry_accessors {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        $(row_accessors! { $field: $ty, $($acc)* })*
    };
}

first_class!(registry_accessors);

optional_component_accessors! {
    PrefabLink => prefab_link, prefab_link_mut, has_prefab_link, set_prefab_link, take_prefab_link, ids_with_prefab_link;
}

impl World {
    /// Visit every Image, in no set order and without allocating: for a per-tick
    /// scan whose answer does not depend on order (`ui::TextureSizeReader`, #1241).
    /// Work whose order matters walks `ids_with_image`.
    pub fn each_image(&self, visit: impl FnMut(&ImageComponent)) {
        self.each_component(visit);
    }

    /// Store skinned mesh `id`'s posed bone palette. Derived from the bones' poses
    /// every tick, so it stamps no change and notes nothing for Undo (`derived_mut`):
    /// a Mesh change cursor (the despawn index's) sees no write (#1708).
    pub fn set_pose_palette(&mut self, id: u32, palette: Vec<glam::Mat4>) {
        if let Some(mut mesh) = self.derived_mut::<MeshComponent>(id) {
            mesh.pose_palette = palette;
        }
    }
}
