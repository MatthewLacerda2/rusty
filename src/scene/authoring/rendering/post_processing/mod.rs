//! src/scene/authoring/rendering/post_processing/ — the post-processing volume's
//! ops, one module per post effect (#1467; the grading and lens effects in their
//! own folders) plus the volume's own settings (`volume`).
//!
//! The ONE place the engine knows how to mutate a `PostProcessingComponent` field by
//! field, with the validation each write owns. The editor's Post Processing card,
//! the Lua `Graphics.*` functions, `Component.Set`, a loaded file's hold (#1418)
//! and a tween all route through these, so they share one write and one clamp
//! (#287). The `Graphics` adapter still parses names (tonemap, modes) into the
//! typed values these ops take.
//!
//! Each effect module declares its [`Effect`]: its field-table row and its numeric
//! [`Knob`]s. [`EFFECTS`] lists them, and the volume's field table ([`FIELDS`]) is
//! assembled from it, so a new effect adds its module, its `mod` line and its row in
//! [`EFFECTS`].
//!
//! Pure.

use crate::components::PostProcessingComponent;
use crate::scene::authoring::fields::table::Field;

// sorted: one module per effect (or family of effects); none reads another
pub mod bloom;
pub mod custom_effects;
pub mod depth_of_field;
pub mod exposure;
pub mod grading;
mod knob;
pub mod lens;
pub mod ssr;
pub mod volume;

pub use knob::{Effect, Knob};

use grading::{color_adjustments, color_lookup, lift_gamma_gain, tonemapping, white_balance};
use lens::{chromatic_aberration, film_grain, vignette};

/// Every post effect's authoring parts, named as the volume names its field.
pub const EFFECTS: &[Effect] = &[
    // sorted: one row per effect; the field table runs them in this order, and no
    // effect's op reads another's settings
    bloom::EFFECT,
    chromatic_aberration::EFFECT,
    color_adjustments::EFFECT,
    color_lookup::EFFECT,
    custom_effects::EFFECT,
    depth_of_field::EFFECT,
    exposure::EFFECT,
    film_grain::EFFECT,
    lift_gamma_gain::EFFECT,
    ssr::EFFECT,
    tonemapping::EFFECT,
    vignette::EFFECT,
    white_balance::EFFECT,
];

/// How many rows the volume's field table has.
const ROWS: usize = volume::FIELDS.len() + EFFECTS.len();

/// The volume's field table (`scene::authoring::fields`): its own settings, then
/// every effect's row, in [`EFFECTS`]' order.
pub const FIELDS: [Field<PostProcessingComponent>; ROWS] = {
    let mut rows = [volume::FIELDS[0]; ROWS];
    let mut i = 0;
    while i < volume::FIELDS.len() {
        rows[i] = volume::FIELDS[i];
        i += 1;
    }
    let mut e = 0;
    while e < EFFECTS.len() {
        rows[i + e] = EFFECTS[e].field;
        e += 1;
    }
    rows
};

/// The effect named `name`.
pub fn effect(name: &str) -> Option<&'static Effect> {
    EFFECTS.iter().find(|e| e.name == name)
}

/// The knob a tween path names after `Graphics.`: `<effect>.<knob>`, the knob's
/// field inside its effect (`bloom.intensity`, `exposure.automatic.compensation`).
pub fn knob(path: &str) -> Option<&'static Knob> {
    let (name, field) = path.split_once('.')?;
    effect(name)?.knob(field)
}

#[cfg(test)]
#[path = "depth_of_field_tests.rs"]
mod depth_of_field_tests;
#[cfg(test)]
mod ops_tests;
#[cfg(test)]
mod tests;
