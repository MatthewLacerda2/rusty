//! src/ecs/access.rs — typed component access on the World (#344).
//!
//! Stage 1 of the megastruct split (#343): every consumer reaches components
//! through these accessors instead of projecting fields out of the stored
//! `Entity` bundle itself. Today the accessors read the megastruct internally —
//! that is the whole point: after this seam, #345 can flip the storage to real
//! per-component hecs columns by rewriting ONLY this module, without touching a
//! single consumer again.
//!
//! Shape rules the seam relies on:
//! - `CompRef`/`CompMut` are opaque guards that deref to the component. Their
//!   internals (a projection out of the bundle today, a native hecs column
//!   borrow tomorrow) are this module's business alone.
//! - At most ONE mutable guard per entity may be live at a time (the bundle is
//!   one hecs component, so mutable borrows can't be split). Consumers read
//!   what they need, drop the guard, then take the next borrow; the rare
//!   genuine split borrows get dedicated `with_…` helpers (see `core.rs`).
//! - Ordered iteration (`ids_with_*`) walks insertion order — the determinism
//!   contract (physics pair ordering, replay byte-identity) that #346's narrow
//!   queries must keep honouring.
//!
//! Allowed deps: hecs, components.

use std::ops::{Deref, DerefMut};

use crate::components::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, ColliderComponent, Entity,
    LightComponent, MaterialComponent, MeshComponent, NavMeshAgentComponent,
    ParticleEmitterComponent, PrefabLink, RigidBodyComponent, VisualCorrectionComponent,
};

use super::world::{Ref, RefMut, World};

/// Shared borrow of one component, projected out of the entity's storage.
pub struct CompRef<'w, T: ?Sized> {
    guard: Ref<'w, Entity>,
    project: fn(&Entity) -> &T,
}

impl<'w, T: ?Sized> CompRef<'w, T> {
    pub(super) fn new(guard: Ref<'w, Entity>, project: fn(&Entity) -> &T) -> Self {
        Self { guard, project }
    }
}

impl<T: ?Sized> Deref for CompRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        (self.project)(&self.guard)
    }
}

/// Exclusive borrow of one component, projected out of the entity's storage.
pub struct CompMut<'w, T: ?Sized> {
    guard: RefMut<'w, Entity>,
    project: fn(&Entity) -> &T,
    project_mut: fn(&mut Entity) -> &mut T,
}

impl<'w, T: ?Sized> CompMut<'w, T> {
    pub(super) fn new(
        guard: RefMut<'w, Entity>,
        project: fn(&Entity) -> &T,
        project_mut: fn(&mut Entity) -> &mut T,
    ) -> Self {
        Self {
            guard,
            project,
            project_mut,
        }
    }
}

impl<T: ?Sized> Deref for CompMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        (self.project)(&self.guard)
    }
}

impl<T: ?Sized> DerefMut for CompMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        (self.project_mut)(&mut self.guard)
    }
}

/// Generate the accessor family for one OPTIONAL first-class component:
/// `get` / `get_mut` (None when absent), `has`, `set` (Some = attach/replace,
/// None = detach; hecs insert/remove after #345), and `ids_with` (stable ids
/// carrying the component, in insertion order).
macro_rules! optional_component_accessors {
    ($($field:ident : $ty:ty => $get:ident, $get_mut:ident, $has:ident, $set:ident, $ids_with:ident;)*) => {
        impl World {
            $(
                pub fn $get(&self, id: u32) -> Option<CompRef<'_, $ty>> {
                    let guard = self.get(id)?;
                    guard.$field.is_some().then(move || {
                        CompRef::new(guard, |e| {
                            e.$field.as_ref().expect("presence checked under this borrow")
                        })
                    })
                }

                pub fn $get_mut(&mut self, id: u32) -> Option<CompMut<'_, $ty>> {
                    let guard = self.get_mut(id)?;
                    guard.$field.is_some().then(move || {
                        CompMut::new(
                            guard,
                            |e| e.$field.as_ref().expect("presence checked under this borrow"),
                            |e| e.$field.as_mut().expect("presence checked under this borrow"),
                        )
                    })
                }

                pub fn $has(&self, id: u32) -> bool {
                    self.get(id).is_some_and(|e| e.$field.is_some())
                }

                /// Attach (`Some`) or detach (`None`) the component. Returns
                /// `false` when the entity does not exist.
                pub fn $set(&mut self, id: u32, value: Option<$ty>) -> bool {
                    match self.get_mut(id) {
                        Some(mut e) => {
                            e.$field = value;
                            true
                        }
                        None => false,
                    }
                }

                /// Stable ids of the entities carrying this component, in
                /// insertion order (the determinism-safe iteration order).
                pub fn $ids_with(&self) -> Vec<u32> {
                    self.ids()
                        .iter()
                        .copied()
                        .filter(|&id| self.get(id).is_some_and(|e| e.$field.is_some()))
                        .collect()
                }
            )*
        }
    };
}

optional_component_accessors! {
    mesh: MeshComponent => mesh, mesh_mut, has_mesh, set_mesh, ids_with_mesh;
    material: MaterialComponent => material, material_mut, has_material, set_material, ids_with_material;
    animator: AnimatorComponent => animator, animator_mut, has_animator, set_animator, ids_with_animator;
    light: LightComponent => light, light_mut, has_light, set_light, ids_with_light;
    collider: ColliderComponent => collider, collider_mut, has_collider, set_collider, ids_with_collider;
    rigidbody: RigidBodyComponent => rigidbody, rigidbody_mut, has_rigidbody, set_rigidbody, ids_with_rigidbody;
    nav_agent: NavMeshAgentComponent => nav_agent, nav_agent_mut, has_nav_agent, set_nav_agent, ids_with_nav_agent;
    camera: CameraComponent => camera, camera_mut, has_camera, set_camera, ids_with_camera;
    visual_correction: VisualCorrectionComponent => visual_correction, visual_correction_mut, has_visual_correction, set_visual_correction, ids_with_visual_correction;
    particles: ParticleEmitterComponent => particles, particles_mut, has_particles, set_particles, ids_with_particles;
    audio: AudioSourceComponent => audio, audio_mut, has_audio, set_audio, ids_with_audio;
    prefab_link: PrefabLink => prefab_link, prefab_link_mut, has_prefab_link, set_prefab_link, ids_with_prefab_link;
}
