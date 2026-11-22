use glam::{Mat4, Vec3};
use serde_json::{json, Value};

mod animator;
mod audio;
mod components;
mod fields;
mod mesh;
mod navigation;
mod ui;

use rusty_components::TransformComponent;
use rusty_ecs::World;
use rusty_scene::Camera;
use rusty_scene::Scene;
use rusty_ui::UiView;
pub(crate) fn vec3(v: Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

pub fn world_value(
    scene: &Scene,
    camera: &Camera,
    frame: u64,
    playing: bool,
    view: &UiView,
) -> Value {
    world_value_with(scene, camera, frame, playing, view, false)
}

pub fn world_value_with(
    scene: &Scene,
    camera: &Camera,
    frame: u64,
    playing: bool,
    view: &UiView,
    include_bones: bool,
) -> Value {
    let bones = scene.bone_ids();
    // Collect ids first so the per-entity accessor borrows below don't overlap
    // any iterator-held guard.
    let ids = scene.entity_ids();
    let entities: Vec<Value> = ids
        .iter()
        .filter(|&&id| scene.world.contains(id))
        .filter(|id| include_bones || !bones.contains(id))
        .map(|&id| {
            let world_matrix = scene.world.local_to_world(id);
            let mut value = entity_value(scene, id, world_matrix, view);
            if bones.contains(&id) {
                value["bone"] = json!(true);
            }
            value
        })
        .collect();
    json!({
        "frame": frame,
        "play_state": if playing { "playing" } else { "editor" },
        "camera": camera_value(camera),
        "entities": entities,
    })
}

fn camera_value(cam: &Camera) -> Value {
    json!({
        "pos": vec3(cam.position),
        "yaw": cam.yaw,
        "pitch": cam.pitch,
        "fov": cam.fov,
    })
}

pub fn entity_value(scene: &Scene, id: u32, world_matrix: Mat4, view: &UiView) -> Value {
    let world = &scene.world;
    let mut value = json!({
        "id": id,
        "name": world.name(id).map(|n| n.clone()).unwrap_or_default(),
        // lint: allow(active-self) the snapshot reports both flags, as Unity does
        "active": world.is_active_self(id),
        "active_in_hierarchy": world.is_active_in_hierarchy(id),
        "static": world.is_static(id),
        "layer": world.layer(id),
        "parent": world.parent_id(id),
        "children": world.children(id),
        "components": inventory(world, id),
        "transform": world.transform(id).map(|t| transform_value(&t)),
        "bounds": bounds_value(scene, id, world_matrix),
        "scripts": world
            .scripts(id)
            .map(|s| s.iter().map(|sc| sc.path.clone()).collect::<Vec<_>>())
            .unwrap_or_default(),
        "ui_rect": rusty_ui::layout::rect_in(world, id, view)
            .map(|r| super::ui::rect_value(world, &r, view)),
    });
    if let Value::Object(map) = &mut value {
        map.extend(
            fields::component_fields(scene, id)
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.unwrap_or(Value::Null))),
        );
    }
    value
}

fn inventory(world: &World, id: u32) -> Vec<&'static str> {
    let mut names = fields::carried(world, id);
    if world.scripts(id).is_some_and(|s| !s.is_empty()) {
        names.push("Script");
    }
    names
}

fn transform_value(t: &TransformComponent) -> Value {
    json!({
        "pos": vec3(t.position),
        "rot": vec3(t.euler_angles()),
        "scale": vec3(t.scale),
    })
}

fn bounds_value(scene: &Scene, id: u32, world_matrix: Mat4) -> Value {
    let aabb = scene
        .world
        .mesh(id)
        .and_then(|m| m.world_aabb(world_matrix))
        .or_else(|| {
            let pose = scene.world.world_pose(id)?;
            scene
                .world
                .collider(id)
                .map(|c| c.calculate_world_aabb(pose))
        });
    match aabb {
        Some((min, max)) => json!({ "min": vec3(min), "max": vec3(max) }),
        None => Value::Null,
    }
}

#[cfg(test)]
mod bone_tests;
#[cfg(test)]
mod tests;
