//! src/api/snapshot/navigation.rs — the NavMeshObstacle snapshot (#456), apart
//! from `components.rs` to keep it under the size cap.

use serde_json::{json, Value};

use super::vec3;
use crate::components::NavMeshObstacleComponent;

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
