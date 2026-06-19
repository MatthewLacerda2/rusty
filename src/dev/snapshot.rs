//! src/dev/snapshot.rs — world → JSON snapshot (what the agent "sees").
//!
//! One observation == one snapshot. The harness/bot path spends a `StepUntil` to
//! skip hundreds of frames, then reads ONE of these. The structured shape itself
//! lives on the API surface in [`crate::api::snapshot`] (the read half of
//! editor↔API parity, #180), so this play-testing entry point and the dev-only
//! `Debug.Snapshot` binding return the exact same rich document — no drift.

use serde_json::Value;

use crate::app::GameWorld;

/// Build the full world snapshot from the live world (frame + play-state envelope,
/// camera, and every entity in the authoring-rich shape).
pub fn snapshot(world: &GameWorld) -> Value {
    crate::api::snapshot::world_value(
        &world.scene().borrow(),
        &world.camera().borrow(),
        world.play_frame(),
        world.is_playing(),
    )
}
