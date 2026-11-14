//! crates/edit/src/authoring/mod.rs — Shared structural-authoring verbs.
//!
//! The one place the engine knows how to change scene *structure*: create an
//! entity (optionally as one of the hierarchy toolbar's primitives), attach or
//! detach a first-class component with the inspector's "Add Component" defaults,
//! and the primitives the Create menu offers (`primitive`: their mesh, light and
//! collider, and the geometry shared with serialization's rehydrate path).
//!
//! BOTH callers route through here so editor and API can never drift: the editor
//! (`hierarchy.rs` create, `inspector/components/add/` add-menu) and the API surface
//! (`api::scene`), which the console REPL and headless session drive against the
//! live edit world. Parenting/destroy/save have their own canonical `Scene` entry
//! points (`set_parent` / `destroy_entity` / `save_to_file`); the structural API
//! verbs call straight into those, so they are not duplicated here.

pub mod ai;
pub mod animator;
pub mod audio;
pub mod components;
pub use rusty_scene::create::defaults;
pub mod dependency;
pub mod environment;
pub mod fields;
pub mod fog;
mod import;
pub mod material;
pub mod nav;
pub mod particles;
pub mod physics;
pub mod rendering;
pub mod ribbon;
pub mod script;
pub mod ui;

// An imported material's asset and attaching a script, authoring's own verbs, under
// the names callers reach them by.
pub use crate::authoring::import::material_asset_from_import;
pub use crate::authoring::script::add_script;
// Re-export the per-component defaults, the primitives and `create_entity` so callers
// (editor add-menu, `Scene.AddComponent` API) reach them through this one entry point;
// they live in `scene::create`, below authoring, because the base builds with them too.
pub use rusty_scene::create::{
    attach_default_material, create_drawn_entity, create_entity, default_animator, default_camera,
    default_collider, default_light, default_material, default_nav_agent, default_post_processing,
    default_rigidbody, primitive_collider, primitive_geometry, primitive_mesh_component,
    primitive_revision, subdivided_geometry, Primitive,
};

// The Add/Remove-Component verbs + their `ComponentKind` enum live in
// `authoring::components` (size-cap split); re-exported so existing
// `authoring::{ComponentKind, add_component, remove_component}` paths still resolve.
pub use crate::authoring::components::{add_component, remove_component, ComponentKind};

// The component-dependency mechanism (#359, rusty's `RequireComponent`) lives in
// `authoring::dependency`; re-exported so the editor Add menu, the camera card's
// remove cascade, the inspector's per-frame reconcile, and scene load all reach the
// one enforcement path through this shared authoring entry point.
pub use crate::authoring::dependency::{
    add_with_requirements, enforce_requirements, has_kind, reconcile_requirements,
    remove_with_cascade, satisfy_requirements,
};

// The prefab structural verbs live in this crate's `prefab` module: extract and
// instantiate (unpacked and linked), and `prefab::{apply, link}` for apply, revert and
// the override list. The base's `scene::prefab_link` holds only what the base reads of
// a link. Re-exported here so the editor and the API reach them through this one
// shared authoring entry point.
pub use crate::prefab::apply::{
    apply_instance_field_to_source, apply_instance_to_source, revert_instance_field,
};
pub use crate::prefab::link::{
    list_instance_overrides, reimport_instance, revert_instance_overrides,
};

// The asset-instantiate verb (#182) lives in `edit::asset_instance` (size-cap
// split); re-exported here so the editor's model inspector and the `Scene.Instantiate`
// asset branch both spawn imported sub-objects through this one shared entry point.
pub use crate::asset_instance::{instantiate_asset, instantiate_imported};
pub use crate::model_instance::instantiate_model;

// The per-field material-mutation ops (#287) live in `material`; the editor card and
// the Lua `Material.*` API are siblings over them, so the field write + validation
// (e.g. the alpha clamp) lives once. `ensure_material_key` is the shared resolve-or-
// create the API uses; callers reach the ops via `authoring::material::*`.
