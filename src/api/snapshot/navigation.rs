//! src/api/snapshot/navigation.rs — the NavMeshObstacle (#456) and OffMeshLink
//! (#462) snapshots, apart from `components.rs` to keep it under the size cap.

use serde_json::{json, Value};

use super::vec3;
use crate::components::{NavMeshObstacleComponent, OffMeshLinkComponent};

/// NavMeshObstacle: its shape, its carving options, and whether it carves now.
pub(crate) fn nav_obstacle_value(o: &NavMeshObstacleComponent) -> Value {
    json!({
        "active": o.active,
        "shape": o.shape.as_str(),
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
    })
}
