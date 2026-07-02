//! src/api/snapshot_components.rs — per-component serializers for the scene-read.
//!
//! The leaf `*_value` builders that turn each first-class component into the stable
//! authoring JSON `snapshot.rs` assembles per entity. Split out of `snapshot.rs` to
//! keep that orchestration file under the size cap; the shape is unchanged.

use serde_json::{json, Value};

use super::snapshot::vec3;
use crate::components::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, ColliderComponent, ColliderShape,
    LightComponent, LightType, MaterialAsset, MeshComponent, NavMeshAgentComponent,
    ParticleEmitterComponent, RigidBodyComponent,
};

/// Mesh identity: the primitive kind and, for imported meshes, the
/// `path::sub_object` asset reference. Never the GPU geometry.
pub(crate) fn mesh_value(m: &MeshComponent) -> Value {
    json!({
        "primitive_type": m.primitive_type,
        "asset_ref": m.asset_ref,
    })
}

/// Material (PBR) params + texture references, read from the resolved library
/// `MaterialAsset`. The albedo map is surfaced under the legacy `"texture"` key
/// (empty string when unset, preserving the prior shape). `render_mode`/`alpha`/
/// `alpha_cutoff` expose the transparency story (#242) so a script can read back what
/// `Material.SetRenderMode`/`SetAlpha`/`SetAlphaCutoff` wrote.
pub(crate) fn material_value(m: &MaterialAsset) -> Value {
    json!({
        "color": m.base_color,
        "metallic": m.metallic,
        "roughness": m.roughness,
        "texture": m.base_color_map.clone().unwrap_or_default(),
        "metallic_map": m.metallic_map,
        "roughness_map": m.roughness_map,
        "normal_map": m.normal_map,
        "emissive": m.emissive,
        "render_mode": format!("{:?}", m.render_mode),
        "alpha": m.alpha,
        "alpha_cutoff": m.alpha_cutoff,
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

pub(crate) fn light_value(l: &LightComponent) -> Value {
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

pub(crate) fn collider_value(c: &ColliderComponent) -> Value {
    json!({
        "active": c.active,
        "is_trigger": c.is_trigger,
        "shape": collider_shape_value(&c.shape),
    })
}

pub(crate) fn rigidbody_value(r: &RigidBodyComponent) -> Value {
    json!({
        "active": r.active,
        "is_kinematic": r.is_kinematic,
        "mass": r.mass,
        "velocity": vec3(r.velocity),
        "use_gravity": r.use_gravity,
    })
}

pub(crate) fn camera_component_value(c: &CameraComponent) -> Value {
    json!({
        "active": c.active,
        "fov": c.fov,
        "near": c.near,
        "far": c.far,
        "culling_mask": c.culling_mask,
        "render_order": c.render_order,
    })
}

pub(crate) fn nav_agent_value(n: &NavMeshAgentComponent) -> Value {
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

pub(crate) fn particle_value(p: &ParticleEmitterComponent) -> Value {
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

/// Animator playback state plus the typed graph parameters (#314), so a script can
/// read back what `Animator.Set*` wrote. Each parameter serializes externally
/// tagged, e.g. `{"Bool": true}` / `{"Trigger": false}`; the `BTreeMap` keeps the
/// keys name-sorted.
pub(crate) fn animator_value(a: &AnimatorComponent) -> Value {
    json!({
        "clip": a.current_clip,
        "time": a.time,
        "speed": a.speed,
        "playing": a.is_playing,
        "loop": a.loop_clip,
        "paused": a.freeze,
        "parameters": a.parameters,
    })
}

/// AudioSource authoring view (#212): the clip + playback flags, plus the spatial
/// fields stored now for #213. The live playing state lives in the `AudioMaestro`
/// roster, not here — this is the persistent component's own data.
pub(crate) fn audio_value(a: &AudioSourceComponent) -> Value {
    json!({
        "clip": a.clip,
        "volume": a.volume,
        "loop": a.looping,
        "play_on_start": a.play_on_start,
        "is_time_scaled": a.is_time_scaled,
        "spatial_blend": a.spatial_blend,
        "initial_distance": a.initial_distance,
        "final_distance": a.final_distance,
    })
}
