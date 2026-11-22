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

pub use crate::authoring::import::material_asset_from_import;
pub use crate::authoring::script::add_script;
pub use rusty_scene::create::{
    attach_default_material, create_drawn_entity, create_entity, default_animator, default_camera,
    default_collider, default_light, default_material, default_nav_agent, default_post_processing,
    default_rigidbody, primitive_collider, primitive_geometry, primitive_mesh_component, Primitive,
};

pub use crate::authoring::components::{add_component, remove_component, ComponentKind};

pub use crate::authoring::dependency::{
    add_with_requirements, enforce_requirements, has_kind, reconcile_requirements,
    remove_with_cascade, satisfy_requirements,
};

pub use crate::prefab::apply::{
    apply_instance_field_to_source, apply_instance_to_source, revert_instance_field,
};
pub use crate::prefab::link::{
    list_instance_overrides, reimport_instance, revert_instance_overrides,
};

pub use crate::asset_instance::{instantiate_asset, instantiate_imported};
pub use crate::model_instance::instantiate_model;
