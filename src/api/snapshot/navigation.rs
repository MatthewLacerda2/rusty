//! src/api/snapshot/navigation.rs — the NavMeshObstacle (#456), OffMeshLink (#462)
//! and NavMeshModifierVolume (#460) snapshots, and AIPerception's (#1900), apart from `components.rs` to keep it
//! under the size cap.

use serde_json::{json, Value};

use super::vec3;
use crate::components::AIPerceptionComponent;
use crate::components::{
    NavMeshModifierVolumeComponent, NavMeshObstacleComponent, OffMeshLinkComponent,
};

/// NavMeshObstacle: its shape, its carving options, and whether it carves now.
pub(crate) fn nav_obstacle_value(o: &NavMeshObstacleComponent) -> Value {
    json!({
        "active": o.active,
        "shape": o.shape.name(),
        "center": vec3(o.center),
        "size": vec3(o.size),
        "radius": o.radius,
        "height": o.height,
        "carve": o.carve,
        "carve_only_stationary": o.carve_only_stationary,
        "move_threshold": o.move_threshold,
        "time_to_stationary": o.time_to_stationary,
        "is_carving": o.is_carving(),
        "velocity": vec3(o.velocity),
    })
}

/// OffMeshLink: its local ends and options.
pub(crate) fn offmesh_link_value(l: &OffMeshLinkComponent) -> Value {
    json!({
        "active": l.active,
        "start": vec3(l.start),
        "end": vec3(l.end),
        "bidirectional": l.bidirectional,
        "cost_override": l.cost_override,
        "area": l.area,
    })
}

/// NavMeshModifierVolume: its local box and the area it assigns.
pub(crate) fn nav_modifier_value(v: &NavMeshModifierVolumeComponent) -> Value {
    json!({
        "active": v.active,
        "center": vec3(v.center),
        "size": vec3(v.size),
        "area": v.area,
    })
}

/// AIPerception: its settings, then what it knows (target ids) and sees now.
pub(crate) fn ai_perception_value(p: &AIPerceptionComponent) -> Value {
    json!({
        "sight_range": p.sight_range,
        "lose_sight_range": p.lose_sight_range,
        "sight_angle": p.sight_angle,
        "eye_height": p.eye_height,
        "sight_mask": p.sight_mask,
        "blocking_mask": p.blocking_mask,
        "hearing_range": p.hearing_range,
        "sight_max_age": p.sight_max_age,
        "hearing_max_age": p.hearing_max_age,
        "damage_max_age": p.damage_max_age,
        "known": p.memory.known.keys().collect::<Vec<_>>(),
        "in_sight": p.memory.in_sight.iter().collect::<Vec<_>>(),
    })
}
