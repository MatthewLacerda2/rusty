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
use crate::components::{Entity, MaterialAsset, TransformComponent};
use crate::render::Camera;
use crate::scene::Scene;

/// A `glam::Vec3` as a `[x, y, z]` JSON array. Shared with `snapshot_components`.
pub(crate) fn vec3(v: Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

/// The whole-world snapshot: play-state envelope + camera + every entity.
pub fn world_value(scene: &Scene, camera: &Camera, frame: u64, playing: bool) -> Value {
    // Collect ids first so the per-entity borrow for the world matrix and the
    // entity read don't overlap the iterator's guards.
    let ids: Vec<u32> = scene.iter().map(|e| e.id).collect();
    let entities: Vec<Value> = ids
        .iter()
        .filter_map(|&id| {
            let world_matrix = scene.compute_world_matrix(id);
            scene.get_entity(id).map(|e| {
                let material = scene.material_of(&e);
                entity_value(&e, material, world_matrix)
            })
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
/// parent-aware world transform, used for the world-space bounds; `material` is the
/// entity's resolved library material (`None` when it references none).
pub fn entity_value(e: &Entity, material: Option<&MaterialAsset>, world_matrix: Mat4) -> Value {
    json!({
        "id": e.id,
        "name": e.name,
        "active": e.active,
        "static": e.is_static,
        "layer": e.layer,
        "parent": e.parent_id,
        "children": e.children,
        "components": inventory(e),
        "transform": transform_value(&e.transform),
        "bounds": bounds_value(e, world_matrix),
        "scripts": e.scripts.iter().map(|s| s.path.clone()).collect::<Vec<_>>(),
        "mesh": e.mesh.as_ref().map(mesh_value),
        "material": material.map(material_value),
        "light": e.light.as_ref().map(light_value),
        "collider": e.collider.as_ref().map(collider_value),
        "rigidbody": e.rigidbody.as_ref().map(rigidbody_value),
        "camera": e.camera.as_ref().map(camera_component_value),
        "nav_agent": e.nav_agent.as_ref().map(nav_agent_value),
        "particles": e.particles.as_ref().map(particle_value),
        "animator": e.animator.as_ref().map(animator_value),
        "audio": e.audio.as_ref().map(audio_value),
    })
}

/// Names of the optional first-class components this entity carries (the
/// "component inventory" authoring needs). `Transform` is mandatory and omitted.
fn inventory(e: &Entity) -> Vec<&'static str> {
    let mut names = Vec::new();
    if e.mesh.is_some() {
        names.push("Mesh");
    }
    if e.material.is_some() {
        names.push("Material");
    }
    if e.light.is_some() {
        names.push("Light");
    }
    if e.collider.is_some() {
        names.push("Collider");
    }
    if e.rigidbody.is_some() {
        names.push("Rigidbody");
    }
    if e.camera.is_some() {
        names.push("Camera");
    }
    if e.nav_agent.is_some() {
        names.push("NavMeshAgent");
    }
    if e.particles.is_some() {
        names.push("ParticleEmitter");
    }
    if e.animator.is_some() {
        names.push("Animator");
    }
    if e.audio.is_some() {
        names.push("AudioSource");
    }
    if !e.scripts.is_empty() {
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
fn bounds_value(e: &Entity, world_matrix: Mat4) -> Value {
    let aabb = e.compute_world_aabb(world_matrix).or_else(|| {
        e.collider
            .as_ref()
            .map(|c| c.calculate_world_aabb(world_matrix))
    });
    match aabb {
        Some((min, max)) => json!({ "min": vec3(min), "max": vec3(max) }),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests;
