//! crates/scene/src/authoring/fields/kinds/ai.rs — the AI components' field tables.

use super::super::table::{inspect, op, with};
use super::super::value::Mask;
use crate::authoring::ai::perception as ops;
use rusty_components::AIPerceptionComponent;

inspect! {
    AIPerceptionComponent => ai_perception, ai_perception_mut, &[
        op!("sight_range", ops::set_sight_range),
        op!("lose_sight_range", ops::set_lose_sight_range),
        op!("sight_angle", ops::set_sight_angle),
        op!("eye_height", ops::set_eye_height),
        with!("sight_mask", |p, v: Mask| ops::set_sight_mask(p, v.0)),
        with!("blocking_mask", |p, v: Mask| ops::set_blocking_mask(p, v.0)),
        op!("hearing_range", ops::set_hearing_range),
        op!("sight_max_age", ops::set_sight_max_age),
        op!("hearing_max_age", ops::set_hearing_max_age),
        op!("damage_max_age", ops::set_damage_max_age),
    ];
}
