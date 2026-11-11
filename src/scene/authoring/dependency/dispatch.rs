//! src/scene/authoring/dependency/dispatch.rs — the per-kind tables.
//!
//! One arm per first-class [`ComponentKind`] for each World-level primitive the
//! dependency verbs compose: probe ([`has_kind`]), attach with defaults
//! ([`set_default`]) and detach raw ([`clear_one`]). All three are generated from
//! the registry (`components::registry`), so a new kind has its arms by
//! construction. Its Add Component default is its `Default`, unless [`added!`]
//! names another.

use crate::components::registry::first_class;
use crate::ecs::World;
use crate::scene::authoring::components::ComponentKind;
use crate::scene::authoring::defaults::{
    default_animator, default_camera, default_collider, default_light, default_material,
    default_nav_agent, default_rigidbody,
};
use crate::scene::authoring::{primitive_mesh_component, Primitive};
use crate::scene::rehydrate::{bounds_of, refit_collider};
use crate::scene::MaterialComponent;

/// Attach one row's component with its Add Component default: the Mesh and the
/// Material through their own verbs, the kinds below through the inspector's
/// default values (`scene::authoring::defaults`), every other kind its `Default`.
macro_rules! added {
    ($w:ident, $id:ident, mesh, $set:ident) => {
        add_box_mesh($w, $id)
    };
    ($w:ident, $id:ident, material, $set:ident) => {
        attach_default_material_world($w, $id)
    };
    ($w:ident, $id:ident, animator, $set:ident) => {
        $w.$set($id, Some(default_animator()))
    };
    ($w:ident, $id:ident, camera, $set:ident) => {
        $w.$set($id, Some(default_camera()))
    };
    ($w:ident, $id:ident, collider, $set:ident) => {
        $w.$set($id, Some(default_collider()))
    };
    ($w:ident, $id:ident, light, $set:ident) => {
        $w.$set($id, Some(default_light()))
    };
    ($w:ident, $id:ident, nav_agent, $set:ident) => {
        $w.$set($id, Some(default_nav_agent()))
    };
    ($w:ident, $id:ident, rigidbody, $set:ident) => {
        $w.$set($id, Some(default_rigidbody()))
    };
    ($w:ident, $id:ident, $field:ident, $set:ident) => {
        $w.$set($id, Some(Default::default()))
    };
}

macro_rules! dispatch {
    ($($field:ident: $ty:ty => $kind:ident [$get_mut:ident $has:ident $set:ident $take:ident $ids:ident];)*) => {
        /// Does entity `id` carry a component of `kind`? The dependency machinery's
        /// presence probe.
        pub fn has_kind(world: &World, id: u32, kind: ComponentKind) -> bool {
            match kind {
                $(ComponentKind::$kind => world.$has(id),)*
            }
        }

        /// Attach `kind` with its Add-Component defaults at the World level,
        /// returning `false` when the entity is missing. Material rides in as a
        /// staged pending asset (folded into the library when the world guard drops)
        /// — the same path the editor Add menu uses.
        pub(crate) fn set_default(world: &mut World, id: u32, kind: ComponentKind) -> bool {
            match kind {
                $(ComponentKind::$kind => added!(world, id, $field, $set),)*
            }
        }

        /// Detach `kind` from entity `id` (raw, no cascade). Returns `false` when the
        /// entity is missing (clearing an absent component is otherwise a no-op
        /// success). A Material drops only the reference; the shared library material
        /// may still be in use.
        pub(crate) fn clear_one(world: &mut World, id: u32, kind: ComponentKind) -> bool {
            match kind {
                $(ComponentKind::$kind => world.$set(id, None),)*
            }
        }
    };
}

first_class!(dispatch);

/// Unity's empty Mesh Filter draws nothing; rusty's Mesh is the Filter and Renderer
/// in one, so it arrives as the menu's default Box. A Mesh-shaped collider follows
/// the entity's Mesh again: its shared mesh goes (#1657) and its box is refit to
/// the Box, as a re-import refits it. An authored collision mesh (#1960) stays, as
/// Unity's `sharedMesh` does when the MeshFilter changes, and keeps sizing the box.
fn add_box_mesh(world: &mut World, id: u32) -> bool {
    let mesh = primitive_mesh_component(Primitive::Box);
    let bounds = bounds_of(mesh.iter().flat_map(|m| &m.vertices).map(|v| v.position));
    if !world.set_mesh(id, mesh) {
        return false;
    }
    if let Some(mut collider) = world.collider_mut(id) {
        collider.shared_mesh = None;
        refit_collider(&mut collider, bounds);
    }
    true
}

/// Attach a default per-entity library material at the World level via staging — the
/// editor Add menu's material path (the World holds no library, so the default asset
/// rides along as `pending_material`, folded in once the guard drops).
fn attach_default_material_world(world: &mut World, id: u32) -> bool {
    if !world.set_material(id, Some(MaterialComponent::for_entity(id))) {
        return false;
    }
    world.stage_pending_material(id, default_material());
    true
}
