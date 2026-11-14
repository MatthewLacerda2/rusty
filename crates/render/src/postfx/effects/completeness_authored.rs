//! The authoring layer as this crate's tests hand it to the gate: built by
//! `scene::authoring`, as the root's test (`tests/post_effect_layers.rs`) builds it.

use super::{engine_from, Layers};
use rusty_scene::authoring::rendering::post_processing::authored;

/// The layers, the authoring one as `scene::authoring` has it.
pub(super) fn engine() -> Layers {
    engine_from(authored())
}
