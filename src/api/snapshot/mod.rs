//! src/api/snapshot/mod.rs — the structured scene-read ("what the agent sees").
//!
//! The read half of the editor↔API parity surface (#180, epic #176). It turns the
//! **live** world into a stable, diffable JSON document rich enough to *author*
//! against: per entity the full transform (incl. scale), the component inventory,
//! the mesh/asset reference, material params, every first-class component's
//! authoring fields, and the world-space bounds for overlap-aware placement —
//! plus the camera and play-state envelope. GPU buffers are never dumped; only
//! references and values, mirroring the `SceneData` document.
//!
//! It lives in `api` (not `dev`) so both callers depend *downward*: the dev
//! harness (`dev::snapshot`) and the dev-only `Debug.Snapshot` binding both call
//! it, and reads stay on the one API surface.

use glam::{Mat4, Vec2, Vec3};
use serde_json::{json, Value};

mod audio;
mod components;
mod fields;
mod navigation;
mod ui;

use crate::components::TransformComponent;
use crate::ecs::World;
use crate::scene::Camera;
use crate::scene::Scene;
use crate::ui::UiView;
/// A `glam::Vec3` as a `[x, y, z]` JSON array. Shared with the `components` and `ui` builders.
pub(crate) fn vec3(v: Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

/// The whole-world snapshot: play-state envelope + camera + every entity. `screen`
/// is the UI's screen size in pixels (for each UI element's computed `ui_rect`).
/// Skeleton bones (#453) are left out — see [`world_value_with`].
pub fn world_value(
    scene: &Scene,
    camera: &Camera,
    frame: u64,
    playing: bool,
    screen: Vec2,
) -> Value {
    world_value_with(scene, camera, frame, playing, screen, false)
}

/// [`world_value`], choosing whether skeleton bones are included (#453): a
/// character's ~65 bones would flood the read, so they are opt-in, and an
/// included bone is marked `"bone": true`.
pub fn world_value_with(
    scene: &Scene,
    camera: &Camera,
    frame: u64,
    playing: bool,
    screen: Vec2,
    include_bones: bool,
) -> Value {
    // Markers are placed through the camera, as the UI lays them out (#429).
    let view = UiView::with_camera(screen, camera.clone());
    let bones = scene.bone_ids();
    // Collect ids first so the per-entity accessor borrows below don't overlap
    // any iterator-held guard.
    let ids = scene.entity_ids();
    let entities: Vec<Value> = ids
        .iter()
        .filter(|&&id| scene.world.contains(id))
        .filter(|id| include_bones || !bones.contains(id))
        .map(|&id| {
            let world_matrix = scene.compute_world_matrix(id);
            let mut value = entity_value(scene, id, world_matrix, &view);
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

/// The active (world) camera — pose + lens.
fn camera_value(cam: &Camera) -> Value {
    json!({
        "pos": vec3(cam.position),
        "yaw": cam.yaw,
        "pitch": cam.pitch,
        "fov": cam.fov,
    })
}

/// One entity in the stable authoring shape. `world_matrix` is the entity's
/// parent-aware world transform, used for the world-space bounds; `view` is the
/// UI's screen and camera, used for the computed `ui_rect` (markers included). Reads route
/// through the `World` component accessors (#344) instead of projecting fields
/// out of a whole-`Entity` borrow, so heavy fields (meshes) are never cloned.
pub fn entity_value(scene: &Scene, id: u32, world_matrix: Mat4, view: &UiView) -> Value {
    let world = &scene.world;
    let mut value = json!({
        "id": id,
        "name": world.name(id).map(|n| n.clone()).unwrap_or_default(),
        "active": world.is_active(id),
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
        "ui_rect": crate::ui::layout::rect_in(world, id, view)
            .map(|r| super::ui::rect_value(world, &r, view)),
    });
    if let Value::Object(map) = &mut value {
        map.extend(
            fields::component_fields(scene, id)
                .map(|(k, v)| (k.to_owned(), v.unwrap_or(Value::Null))),
        );
    }
    value
}

/// Names of the optional first-class components this entity carries (the
/// "component inventory" authoring needs). `Transform` is mandatory and omitted.
fn inventory(world: &World, id: u32) -> Vec<&'static str> {
    type Probe = fn(&World, u32) -> bool;
    let probes: &[(Probe, &'static str)] = &[
        (World::has_mesh, "Mesh"),
        (World::has_material, "Material"),
        (World::has_light, "Light"),
        (World::has_collider, "Collider"),
        (World::has_rigidbody, "Rigidbody"),
        (World::has_camera, "Camera"),
        (World::has_nav_agent, "NavMeshAgent"),
        (World::has_nav_obstacle, "NavMeshObstacle"),
        (World::has_offmesh_link, "OffMeshLink"),
        (World::has_nav_modifier, "NavMeshModifierVolume"),
        (World::has_particles, "ParticleEmitter"),
        (World::has_animator, "Animator"),
        (World::has_audio, "AudioSource"),
        (World::has_reverb_zone, "AudioReverbZone"),
        (World::has_canvas, "Canvas"),
        (World::has_rect_transform, "RectTransform"),
        (World::has_image, "Image"),
        (World::has_canvas_group, "CanvasGroup"),
        (World::has_rect_mask, "RectMask"),
        (World::has_mask, "Mask"),
        (World::has_backdrop_filter, "BackdropFilter"),
        (World::has_text, "Text"),
        (World::has_shape, "Shape"),
        (World::has_selectable, "Selectable"),
        (World::has_layout_group, "LayoutGroup"),
        (World::has_layout_element, "LayoutElement"),
        (World::has_joint, "Joint"),
        (World::has_character_controller, "CharacterController"),
        (World::has_lod_group, "LODGroup"),
        (World::has_trail, "TrailRenderer"),
        (World::has_line, "LineRenderer"),
        (
            |w, id| w.scripts(id).is_some_and(|s| !s.is_empty()),
            "Script",
        ),
    ];
    probes
        .iter()
        .filter(|(has, _)| has(world, id))
        .map(|&(_, name)| name)
        .collect()
}

/// Full transform — position, Euler rotation (degrees), and **scale**.
fn transform_value(t: &TransformComponent) -> Value {
    json!({
        "pos": vec3(t.position),
        "rot": vec3(t.euler_angles()),
        "scale": vec3(t.scale),
    })
}

/// World-space AABB for overlap-aware placement: the mesh bounds when present,
/// else the collider bounds, else `null`.
fn bounds_value(scene: &Scene, id: u32, world_matrix: Mat4) -> Value {
    let aabb = scene
        .world
        .mesh(id)
        .and_then(|m| m.world_aabb(world_matrix))
        .or_else(|| {
            scene
                .world
                .collider(id)
                .map(|c| c.calculate_world_aabb(world_matrix))
        });
    match aabb {
        Some((min, max)) => json!({ "min": vec3(min), "max": vec3(max) }),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests;
