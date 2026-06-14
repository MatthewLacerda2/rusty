//! src/dev/snapshot.rs — world → JSON snapshot (what the agent "sees").
//!
//! One observation == one snapshot. The agent spends a `StepUntil` to skip hundreds
//! of frames, then reads ONE of these. Fields are intentionally small and stable:
//! per-entity id/name/pos/rot/health/clip, plus camera and play-state.

use serde_json::{json, Value};

use crate::app::GameWorld;

fn vec3(v: glam::Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

/// Snapshot a single entity into the stable observation shape.
fn entity_json(e: &crate::core::scene::Entity) -> Value {
    let health = e
        .health
        .as_ref()
        .map(|h| json!({ "current": h.current_health, "max": h.max_health, "dead": h.is_dead }));
    let clip = e.animator.as_ref().map(|a| a.current_clip.clone());
    json!({
        "id": e.id,
        "name": e.name,
        "active": e.active,
        "pos": vec3(e.transform.position),
        "rot": vec3(e.transform.euler_angles()),
        "health": health,
        "clip": clip,
    })
}

/// Build the full world snapshot.
pub fn snapshot(world: &GameWorld) -> Value {
    let scene = world.scene.borrow();
    let entities: Vec<Value> = scene.iter().map(|e| entity_json(&e)).collect();
    json!({
        "frame": world.play_frame(),
        "play_state": if world.is_playing { "playing" } else { "editor" },
        "camera": {
            "pos": vec3(world.camera.position),
            "yaw": world.camera.yaw,
            "pitch": world.camera.pitch,
        },
        "entities": entities,
    })
}
