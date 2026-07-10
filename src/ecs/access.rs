//! src/ecs/access.rs — typed component access on the World (#344, #345).
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
//! - Ordered iteration (`ids_with_*`) walks insertion order — the determinism
//!   contract (physics pair ordering, replay byte-identity) #346's narrow
//!   queries must keep honouring.
//!
//! Allowed deps: hecs, components.

use std::ops::{Deref, DerefMut};

use crate::components::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, ColliderComponent, LightComponent,
    MaterialComponent, MeshComponent, NavMeshAgentComponent, ParticleEmitterComponent, PrefabLink,
    RigidBodyComponent, VisualCorrectionComponent,
};

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
                pub fn $ids_with(&self) -> Vec<u32> {
                    self.ids().iter().copied().filter(|&id| self.$has(id)).collect()
                }
            )*
        }
    };
}

optional_component_accessors! {
    MeshComponent => mesh, mesh_mut, has_mesh, set_mesh, take_mesh, ids_with_mesh;
    MaterialComponent => material, material_mut, has_material, set_material, take_material, ids_with_material;
    AnimatorComponent => animator, animator_mut, has_animator, set_animator, take_animator, ids_with_animator;
    LightComponent => light, light_mut, has_light, set_light, take_light, ids_with_light;
    ColliderComponent => collider, collider_mut, has_collider, set_collider, take_collider, ids_with_collider;
    RigidBodyComponent => rigidbody, rigidbody_mut, has_rigidbody, set_rigidbody, take_rigidbody, ids_with_rigidbody;
    NavMeshAgentComponent => nav_agent, nav_agent_mut, has_nav_agent, set_nav_agent, take_nav_agent, ids_with_nav_agent;
    CameraComponent => camera, camera_mut, has_camera, set_camera, take_camera, ids_with_camera;
    VisualCorrectionComponent => visual_correction, visual_correction_mut, has_visual_correction, set_visual_correction, take_visual_correction, ids_with_visual_correction;
    ParticleEmitterComponent => particles, particles_mut, has_particles, set_particles, take_particles, ids_with_particles;
    AudioSourceComponent => audio, audio_mut, has_audio, set_audio, take_audio, ids_with_audio;
    PrefabLink => prefab_link, prefab_link_mut, has_prefab_link, set_prefab_link, take_prefab_link, ids_with_prefab_link;
}
