//! src/api/snapshot.rs — the structured scene-read ("what the agent sees").
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

use glam::{Mat4, Vec3};
use serde_json::{json, Value};

use super::snapshot_components::{
    animator_value, audio_value, camera_component_value, collider_value, light_value,
    material_value, mesh_value, nav_agent_value, particle_value, rigidbody_value,
};
use crate::components::TransformComponent;
use crate::ecs::World;
use crate::render::Camera;
use crate::scene::Scene;

/// A `glam::Vec3` as a `[x, y, z]` JSON array. Shared with `snapshot_components`.
pub(crate) fn vec3(v: Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

/// The whole-world snapshot: play-state envelope + camera + every entity.
pub fn world_value(scene: &Scene, camera: &Camera, frame: u64, playing: bool) -> Value {
    // Collect ids first so the per-entity accessor borrows below don't overlap
    // any iterator-held guard.
    let ids = scene.entity_ids();
    let entities: Vec<Value> = ids
        .iter()
        .filter(|&&id| scene.world.contains(id))
        .map(|&id| {
            let world_matrix = scene.compute_world_matrix(id);
            entity_value(scene, id, world_matrix)
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
/// parent-aware world transform, used for the world-space bounds. Reads route
/// through the `World` component accessors (#344) instead of projecting fields
/// out of a whole-`Entity` borrow, so heavy fields (meshes) are never cloned.
pub fn entity_value(scene: &Scene, id: u32, world_matrix: Mat4) -> Value {
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
    })
}

/// Names of the optional first-class components this entity carries (the
/// "component inventory" authoring needs). `Transform` is mandatory and omitted.
fn inventory(world: &World, id: u32) -> Vec<&'static str> {
    let mut names = Vec::new();
    if world.has_mesh(id) {
        names.push("Mesh");
    }
    if world.has_material(id) {
        names.push("Material");
    }
    if world.has_light(id) {
        names.push("Light");
    }
    if world.has_collider(id) {
        names.push("Collider");
    }
    if world.has_rigidbody(id) {
        names.push("Rigidbody");
    }
    if world.has_camera(id) {
        names.push("Camera");
    }
    if world.has_nav_agent(id) {
        names.push("NavMeshAgent");
    }
    if world.has_particles(id) {
        names.push("ParticleEmitter");
    }
    if world.has_animator(id) {
        names.push("Animator");
    }
    if world.has_audio(id) {
        names.push("AudioSource");
    }
    if world.scripts(id).is_some_and(|s| !s.is_empty()) {
        names.push("Script");
    }
    names
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
