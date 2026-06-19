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

use crate::components::{
    AnimatorComponent, CameraComponent, ColliderComponent, ColliderShape, Entity, HealthComponent,
    LightComponent, LightType, MeshComponent, NavMeshAgentComponent, ParticleEmitterComponent,
    RigidBodyComponent, TextureComponent, TransformComponent,
};
use crate::render::Camera;
use crate::scene::Scene;

/// A `glam::Vec3` as a `[x, y, z]` JSON array.
fn vec3(v: Vec3) -> Value {
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
            scene.get_entity(id).map(|e| entity_value(&e, world_matrix))
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
/// parent-aware world transform, used for the world-space bounds.
pub fn entity_value(e: &Entity, world_matrix: Mat4) -> Value {
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
        "material": e.texture.as_ref().map(material_value),
        "light": e.light.as_ref().map(light_value),
        "collider": e.collider.as_ref().map(collider_value),
        "rigidbody": e.rigidbody.as_ref().map(rigidbody_value),
        "camera": e.camera.as_ref().map(camera_component_value),
        "nav_agent": e.nav_agent.as_ref().map(nav_agent_value),
        "particles": e.particles.as_ref().map(particle_value),
        "animator": e.animator.as_ref().map(animator_value),
        "health": e.health.as_ref().map(health_value),
    })
}

/// Names of the optional first-class components this entity carries (the
/// "component inventory" authoring needs). `Transform` is mandatory and omitted.
fn inventory(e: &Entity) -> Vec<&'static str> {
    let mut names = Vec::new();
    if e.mesh.is_some() {
        names.push("Mesh");
    }
    if e.texture.is_some() {
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
    if e.health.is_some() {
        names.push("Health");
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

/// Mesh identity: the primitive kind and, for imported meshes, the
/// `path::sub_object` asset reference. Never the GPU geometry.
fn mesh_value(m: &MeshComponent) -> Value {
    json!({
        "primitive_type": m.primitive_type,
        "asset_ref": m.asset_ref,
    })
}

/// Material (PBR) params + texture references.
fn material_value(t: &TextureComponent) -> Value {
    json!({
        "color": t.color,
        "metallic": t.metallic,
        "roughness": t.roughness,
        "texture": t.path,
        "metallic_map": t.metallic_map,
        "roughness_map": t.roughness_map,
    })
}

/// Light kind as a stable string.
fn light_type_str(kind: &LightType) -> &'static str {
    match kind {
        LightType::Ambient => "Ambient",
        LightType::Directional => "Directional",
        LightType::Point => "Point",
        LightType::Spotlight => "Spotlight",
    }
}

fn light_value(l: &LightComponent) -> Value {
    json!({
        "type": light_type_str(&l.light_type),
        "color": vec3(l.color),
        "intensity": l.intensity,
        "range": l.range,
        "inner_cone": l.inner_cone,
        "outer_cone": l.outer_cone,
    })
}

/// Collider shape with its defining dimensions.
fn collider_shape_value(shape: &ColliderShape) -> Value {
    match shape {
        ColliderShape::Box { size } => json!({ "kind": "Box", "size": vec3(*size) }),
        ColliderShape::Sphere { radius } => json!({ "kind": "Sphere", "radius": radius }),
        ColliderShape::Cylinder { radius, height } => {
            json!({ "kind": "Cylinder", "radius": radius, "height": height })
        }
        ColliderShape::Mesh {
            convex,
            local_min,
            local_max,
        } => json!({
            "kind": "Mesh",
            "convex": convex,
            "local_min": vec3(*local_min),
            "local_max": vec3(*local_max),
        }),
    }
}

fn collider_value(c: &ColliderComponent) -> Value {
    json!({
        "active": c.active,
        "is_trigger": c.is_trigger,
        "shape": collider_shape_value(&c.shape),
    })
}

fn rigidbody_value(r: &RigidBodyComponent) -> Value {
    json!({
        "active": r.active,
        "is_kinematic": r.is_kinematic,
        "mass": r.mass,
        "velocity": vec3(r.velocity),
        "use_gravity": r.use_gravity,
    })
}

fn camera_component_value(c: &CameraComponent) -> Value {
    json!({
        "active": c.active,
        "fov": c.fov,
        "near": c.near,
        "far": c.far,
        "culling_mask": c.culling_mask,
        "render_order": c.render_order,
    })
}

fn nav_agent_value(n: &NavMeshAgentComponent) -> Value {
    json!({
        "active": n.active,
        "radius": n.radius,
        "target": vec3(n.target),
        "speed": n.speed,
        "acceleration": n.acceleration,
        "stopping_distance": n.stopping_distance,
        "velocity": vec3(n.velocity),
    })
}

fn particle_value(p: &ParticleEmitterComponent) -> Value {
    json!({
        "active": p.active,
        "texture": p.texture,
        "rate": p.rate,
        "max_particles": p.max_particles,
        "lifetime": p.lifetime,
        "speed": p.speed,
        "direction": vec3(p.direction),
        "color": p.color,
    })
}

fn animator_value(a: &AnimatorComponent) -> Value {
    json!({
        "clip": a.current_clip,
        "time": a.time,
        "speed": a.speed,
        "playing": a.is_playing,
    })
}

fn health_value(h: &HealthComponent) -> Value {
    json!({
        "current": h.current_health,
        "max": h.max_health,
        "dead": h.is_dead,
    })
}

#[cfg(test)]
mod tests;
