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

mod components;
mod ui;

use crate::components::TransformComponent;
use crate::ecs::World;
use crate::scene::Camera;
use crate::scene::Scene;
use crate::ui::UiView;
use components::{
    animator_value, audio_value, camera_component_value, collider_value, light_value,
    material_value, mesh_value, nav_agent_value, particle_value, rigidbody_value,
};
use components::{joint_value, lod_group_value};
use ui::{
    canvas_group_value, canvas_value, image_value, layout_element_value, layout_group_value,
    rect_mask_value, rect_transform_value, selectable_value, text_value,
};

/// A `glam::Vec3` as a `[x, y, z]` JSON array. Shared with the `components` and `ui` builders.
pub(crate) fn vec3(v: Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

/// The whole-world snapshot: play-state envelope + camera + every entity. `screen`
/// is the UI's screen size in pixels (for each UI element's computed `ui_rect`).
pub fn world_value(
    scene: &Scene,
    camera: &Camera,
    frame: u64,
    playing: bool,
    screen: Vec2,
) -> Value {
    // Markers are placed through the camera, as the UI lays them out (#429).
    let view = UiView::with_camera(screen, camera.clone());
    // Collect ids first so the per-entity accessor borrows below don't overlap
    // any iterator-held guard.
    let ids = scene.entity_ids();
    let entities: Vec<Value> = ids
        .iter()
        .filter(|&&id| scene.world.contains(id))
        .map(|&id| {
            let world_matrix = scene.compute_world_matrix(id);
            entity_value(scene, id, world_matrix, &view)
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
    let material = scene.material_asset_of(id);
    json!({
        "id": id,
        "name": world.name(id).map(|n| n.clone()).unwrap_or_default(),
        "active": world.is_active(id),
        "static": world.is_static(id),
        "layer": world.layer(id),
        "parent": world.parent_id(id),
        "children": world.children(id),
        "components": inventory(world, id),
        "transform": world
            .transform(id)
            .map(|t| transform_value(&t))
            .unwrap_or(Value::Null),
        "bounds": bounds_value(scene, id, world_matrix),
        "scripts": world
            .scripts(id)
            .map(|s| s.iter().map(|sc| sc.path.clone()).collect::<Vec<_>>())
            .unwrap_or_default(),
        "mesh": world.mesh(id).map(|m| mesh_value(&m)),
        "material": material.map(material_value),
        "light": world.light(id).map(|l| light_value(&l)),
        "collider": world.collider(id).map(|c| collider_value(&c)),
        "rigidbody": world.rigidbody(id).map(|r| rigidbody_value(&r)),
        "camera": world.camera(id).map(|c| camera_component_value(&c)),
        "nav_agent": world.nav_agent(id).map(|n| nav_agent_value(&n)),
        "particles": world.particles(id).map(|p| particle_value(&p)),
        "animator": world.animator(id).map(|a| animator_value(&a)),
        "audio": world.audio(id).map(|a| audio_value(&a)),
        "canvas": world.canvas(id).map(|c| canvas_value(&c)),
        "rect_transform": world.rect_transform(id).map(|r| rect_transform_value(&r)),
        "image": world.image(id).map(|i| image_value(&i)),
        "canvas_group": world.canvas_group(id).map(|g| canvas_group_value(&g)),
        "rect_mask": world.rect_mask(id).map(|m| rect_mask_value(&m)),
        "text": world.text(id).map(|t| text_value(&t)),
        "selectable": world.selectable(id).map(|s| selectable_value(&s)),
        "layout_group": world.layout_group(id).map(|g| layout_group_value(&g)),
        "layout_element": world.layout_element(id).map(|e| layout_element_value(&e)),
        "joint": world.joint(id).map(|j| joint_value(&j)),
        "lod_group": world.lod_group(id).map(|g| lod_group_value(&g)),
        "ui_rect": crate::ui::layout::rect_in(world, id, view)
            .map(|r| super::ui::rect_value(world, &r, view)),
    })
}

/// Names of the optional first-class components this entity carries (the
/// "component inventory" authoring needs). `Transform` is mandatory and omitted.
fn inventory(world: &World, id: u32) -> Vec<&'static str> {
    type Probe = fn(&World, u32) -> bool;
    let probes: [(Probe, &'static str); 22] = [
        (World::has_mesh, "Mesh"),
        (World::has_material, "Material"),
        (World::has_light, "Light"),
        (World::has_collider, "Collider"),
        (World::has_rigidbody, "Rigidbody"),
        (World::has_camera, "Camera"),
        (World::has_nav_agent, "NavMeshAgent"),
        (World::has_particles, "ParticleEmitter"),
        (World::has_animator, "Animator"),
        (World::has_audio, "AudioSource"),
        (World::has_canvas, "Canvas"),
        (World::has_rect_transform, "RectTransform"),
        (World::has_image, "Image"),
        (World::has_canvas_group, "CanvasGroup"),
        (World::has_rect_mask, "RectMask"),
        (World::has_text, "Text"),
        (World::has_selectable, "Selectable"),
        (World::has_layout_group, "LayoutGroup"),
        (World::has_layout_element, "LayoutElement"),
        (World::has_joint, "Joint"),
        (World::has_lod_group, "LODGroup"),
        (
            |w, id| w.scripts(id).is_some_and(|s| !s.is_empty()),
            "Script",
        ),
    ];
    probes
        .into_iter()
        .filter(|(has, _)| has(world, id))
        .map(|(_, name)| name)
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
