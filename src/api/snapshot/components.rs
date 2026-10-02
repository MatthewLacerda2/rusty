//! src/api/snapshot/components.rs — per-component serializers for the scene-read.
//!
//! The leaf `*_value` builders that turn each first-class component into the stable
//! authoring JSON `snapshot.rs` assembles per entity. Split out of `snapshot.rs` to
//! keep that orchestration file under the size cap; the shape is unchanged.

use serde_json::{json, Value};

use super::vec3;
use crate::components::{
    AnimatorComponent, CameraComponent, ColliderComponent, ColliderShape, LayerState,
    LightComponent, LightType, MaterialAsset, MeshComponent, NavMeshAgentComponent,
    ParticleEmitterComponent, RigidBodyComponent,
};
use crate::components::{
    CharacterControllerComponent, JointComponent, LineComponent, LodGroupComponent, RibbonStyle,
    TrailComponent,
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
/// `Material.SetRenderMode`/`SetAlpha`/`SetAlphaCutoff` wrote; `shader` is the
/// authored surface shader's name (#396), `null` for the standard one;
/// `shader_params` the runtime param values set on it (#399), by canonical name;
/// `shader_textures` the texture each extra shader slot samples (#400).
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
        "shader": m.shader,
        "shader_params": m.shader_params,
        "shader_textures": m.shader_textures,
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
        "cast_shadows": l.cast_shadows,
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
        ColliderShape::Capsule {
            radius,
            height,
            axis,
        } => json!({
            "kind": "Capsule",
            "radius": radius,
            "height": height,
            "axis": axis.as_str(),
        }),
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
        "material": {
            "friction": c.material.friction,
            "bounciness": c.material.bounciness,
            "friction_combine": c.material.friction_combine.as_str(),
            "bounce_combine": c.material.bounce_combine.as_str(),
        },
    })
}

pub(crate) fn rigidbody_value(r: &RigidBodyComponent) -> Value {
    json!({
        "active": r.active,
        "is_kinematic": r.is_kinematic,
        "mass": r.mass,
        "velocity": vec3(r.velocity),
        "angular_velocity": vec3(r.angular_velocity),
        "use_gravity": r.use_gravity,
        "collision_detection": r.collision_detection.as_str(),
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
        "projection": crate::scene::authoring::camera::projection_name(c.projection),
        "target_texture": c.target_texture.as_ref().map(|t| t.path()),
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
        "base_offset": n.base_offset,
        "velocity": vec3(n.velocity),
        "avoidance_priority": n.avoidance_priority,
        "avoidance_enabled": n.avoidance_enabled,
        "auto_traverse_off_mesh_link": n.auto_traverse_off_mesh_link,
        "on_off_mesh_link": n.off_mesh_link.is_some(),
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
        "shape": p.shape.name(),
        "emit_from": p.emit_from,
        "size": p.size,
        "sub_emitters": p.sub_emitters,
    })
}

/// Animator playback state plus the typed graph parameters (#314) and the
/// animation-graph binding (#316), so a script can read back what `Animator.Set*`
/// wrote and which graph state the evaluator holds active. Each parameter
/// serializes externally tagged, e.g. `{"Bool": true}` / `{"Trigger": false}`; the
/// `BTreeMap` keeps the keys name-sorted. `layers` lists the extra layers (#457).
pub(crate) fn animator_value(a: &AnimatorComponent) -> Value {
    json!({
        "clip": a.base.current_clip,
        "time": a.base.time,
        "speed": a.speed,
        "playing": a.is_playing,
        "loop": a.base.loop_clip,
        "paused": a.freeze,
        "parameters": a.parameters,
        "graph": a.graph,
        "graph_enabled": a.graph_enabled,
        "node": a.base.current_node,
        "layers": a.layers.iter().map(layer_value).collect::<Vec<_>>(),
    })
}
fn layer_value(l: &LayerState) -> Value {
    json!({ "name": l.name, "weight": l.weight, "node": l.playback.current_node })
}

/// Joint (#449): its kind, connected body (`null`: the world), anchors, axes,
/// limits (degrees), break thresholds and collision flag.
pub(crate) fn joint_value(j: &JointComponent) -> Value {
    json!({
        "kind": j.kind.name(),
        "connected_body": j.connected_body,
        "anchor": vec3(j.anchor),
        "connected_anchor": vec3(j.connected_anchor),
        "auto_configure_connected_anchor": j.auto_configure_connected_anchor,
        "axis": vec3(j.axis),
        "swing_axis": vec3(j.swing_axis),
        "use_limits": j.use_limits,
        "limits": [j.limits.x, j.limits.y],
        "swing_limit": j.swing_limit,
        "swing2_limit": j.swing2_limit,
        "break_force": j.break_force,
        "break_torque": j.break_torque,
        "enable_collision": j.enable_collision,
    })
}

/// CharacterController (#451): the capsule, its tuning, and what the last `Move`
/// found.
pub(crate) fn character_controller_value(c: &CharacterControllerComponent) -> Value {
    json!({
        "height": c.height,
        "radius": c.radius,
        "center": vec3(c.center),
        "step_offset": c.step_offset,
        "slope_limit": c.slope_limit,
        "skin_width": c.skin_width,
        "min_move_distance": c.min_move_distance,
        "is_grounded": c.is_grounded,
        "collision_flags": c.collision_flags,
        "ground_normal": vec3(c.ground_normal),
    })
}

/// LODGroup (#472): its size and levels, finest first — each level's screen height
/// and renderer ids.
pub(crate) fn lod_group_value(g: &LodGroupComponent) -> Value {
    let levels: Vec<Value> = g
        .levels
        .iter()
        .map(|l| json!({ "screen_height": l.screen_height, "renderers": l.renderers }))
        .collect();
    json!({ "size": g.size, "levels": levels })
}

/// Trail (#441): its settings and how many points it has recorded right now.
pub(crate) fn trail_value(t: &TrailComponent) -> Value {
    json!({
        "emitting": t.emitting,
        "time": t.time,
        "min_vertex_distance": t.min_vertex_distance,
        "position_count": t.runtime.points.len(),
        "style": style_value(&t.style),
    })
}

/// Line (#441): its points, their space and the loop flag.
pub(crate) fn line_value(l: &LineComponent) -> Value {
    json!({
        "positions": l.positions.iter().map(|&p| vec3(p)).collect::<Vec<_>>(),
        "use_world_space": l.use_world_space,
        "loop": l.looping,
        "style": style_value(&l.style),
    })
}

/// The look a trail and a line share: width curve, colour gradient, texture, blend.
fn style_value(s: &RibbonStyle) -> Value {
    json!({
        "width": s.width,
        "color": s.color,
        "texture": s.texture,
        "texture_mode": s.texture_mode.name(),
        "blend": s.blend,
    })
}
