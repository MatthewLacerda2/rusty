//! The authoring layer as this crate's tests hand it to the gate: built by
//! `edit::authoring`, as the root's test (`tests/post_effect_layers.rs`) builds it.

use super::{engine_from, Layers};
use rusty_edit::authoring::rendering::post_processing::authored;

/// The layers, the authoring one as `edit::authoring` has it.
pub(super) fn engine() -> Layers {
    engine_from(authored())
}
