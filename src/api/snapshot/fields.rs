//! src/api/snapshot/fields.rs — every first-class component's snapshot field.
//!
//! A table of `(key, value)` pairs merged into the entity object, not keys of one
//! `json!` literal: that many keys overrun the macro's recursion limit. Grouped by
//! area so each builder stays short.

use serde_json::Value;

use super::audio::{audio_value, reverb_zone_value};
use super::components::character_controller_value as cc_value;
use super::components::{
    animator_value, camera_component_value, collider_value, light_value, material_value,
    mesh_value, nav_agent_value, particle_value, rigidbody_value,
};
use super::components::{joint_value, line_value, lod_group_value, trail_value};
use super::navigation::{nav_obstacle_value, offmesh_link_value};
use super::ui::{backdrop_filter_value, mask_value};
use super::ui::{
    canvas_group_value, canvas_value, image_value, layout_element_value, layout_group_value,
    rect_mask_value, rect_transform_value, selectable_value, shape_value, text_value,
};
use crate::scene::Scene;

type Field = (&'static str, Option<Value>);

/// Each first-class component's authoring fields, keyed by its snapshot name
/// (`None` when the entity lacks it, written as `null`).
pub(super) fn component_fields(scene: &Scene, id: u32) -> impl Iterator<Item = Field> {
    world_fields(scene, id)
        .into_iter()
        .chain(rig_fields(scene, id))
        .chain(ui_fields(scene, id))
}

/// Rendering, physics, navigation, animation and audio.
fn world_fields(scene: &Scene, id: u32) -> [Field; 13] {
    let w = &scene.world;
    [
        ("mesh", w.mesh(id).map(|m| mesh_value(&m))),
        ("material", scene.material_asset_of(id).map(material_value)),
        ("light", w.light(id).map(|l| light_value(&l))),
        ("collider", w.collider(id).map(|c| collider_value(&c))),
        ("rigidbody", w.rigidbody(id).map(|r| rigidbody_value(&r))),
        ("camera", w.camera(id).map(|c| camera_component_value(&c))),
        ("nav_agent", w.nav_agent(id).map(|n| nav_agent_value(&n))),
        (
            "nav_obstacle",
            w.nav_obstacle(id).map(|o| nav_obstacle_value(&o)),
        ),
        (
            "offmesh_link",
            w.offmesh_link(id).map(|l| offmesh_link_value(&l)),
        ),
        ("particles", w.particles(id).map(|p| particle_value(&p))),
        ("animator", w.animator(id).map(|a| animator_value(&a))),
        ("audio", w.audio(id).map(|a| audio_value(&a))),
        (
            "reverb_zone",
            w.reverb_zone(id).map(|z| reverb_zone_value(&z)),
        ),
    ]
}

/// Joints, character controllers, LOD groups and the ribbon renderers.
fn rig_fields(scene: &Scene, id: u32) -> [Field; 5] {
    let w = &scene.world;
    [
        ("joint", w.joint(id).map(|j| joint_value(&j))),
        (
            "character_controller",
            w.character_controller(id).map(|c| cc_value(&c)),
        ),
        ("lod_group", w.lod_group(id).map(|g| lod_group_value(&g))),
        ("trail", w.trail(id).map(|t| trail_value(&t))),
        ("line", w.line(id).map(|l| line_value(&l))),
    ]
}

/// The in-game UI components.
fn ui_fields(scene: &Scene, id: u32) -> [Field; 12] {
    let w = &scene.world;
    [
        ("canvas", w.canvas(id).map(|c| canvas_value(&c))),
        (
            "rect_transform",
            w.rect_transform(id).map(|r| rect_transform_value(&r)),
        ),
        ("image", w.image(id).map(|i| image_value(&i))),
        (
            "canvas_group",
            w.canvas_group(id).map(|g| canvas_group_value(&g)),
        ),
        ("rect_mask", w.rect_mask(id).map(|m| rect_mask_value(&m))),
        ("mask", w.mask(id).map(|m| mask_value(&m))),
        (
            "backdrop_filter",
            w.backdrop_filter(id).map(|b| backdrop_filter_value(&b)),
        ),
        ("text", w.text(id).map(|t| text_value(&t))),
        ("shape", w.shape(id).map(|s| shape_value(&s))),
        ("selectable", w.selectable(id).map(|s| selectable_value(&s))),
        (
            "layout_group",
            w.layout_group(id).map(|g| layout_group_value(&g)),
        ),
        (
            "layout_element",
            w.layout_element(id).map(|e| layout_element_value(&e)),
        ),
    ]
}
