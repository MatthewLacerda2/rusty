use crate::authoring::fields::table::Field;
use rusty_components::PostProcessingComponent;

// sorted: one module per effect (or family of effects); none reads another
pub mod bloom;
pub mod custom_effects;
pub mod depth_of_field;
pub mod exposure;
pub mod grading;
mod knob;
pub mod lens;
pub mod lens_flare;
pub mod ssil;
pub mod ssr;
pub mod volume;
pub mod volumetric_fog;

pub use knob::{Effect, Knob};

use grading::{color_adjustments, color_lookup, lift_gamma_gain, tonemapping, white_balance};
use lens::{chromatic_aberration, film_grain, vignette};

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
    lens_flare::EFFECT,
    lift_gamma_gain::EFFECT,
    ssil::EFFECT,
    ssr::EFFECT,
    tonemapping::EFFECT,
    vignette::EFFECT,
    volumetric_fog::EFFECT,
    white_balance::EFFECT,
];

pub fn authored() -> rusty_scene::authored::Authored {
    let fields_of = |kind: &str| -> Vec<String> {
        let kind = crate::authoring::fields::find(kind).expect("a first-class component");
        (kind.fields)()
            .into_iter()
            .map(|(n, _)| n.to_string())
            .collect()
    };
    let names = |fields: &mut dyn Iterator<Item = &str>| fields.map(String::from).collect();
    use super::camera::{TAA_EFFECT, TAA_KNOBS};
    rusty_scene::authored::Authored {
        volume_fields: fields_of("PostProcessing"),
        camera_fields: fields_of("Camera"),
        ops: (EFFECTS.iter())
            .map(|e| (e.name.into(), names(&mut e.knobs.iter().map(|k| k.field))))
            .collect(),
        camera_knobs: vec![(
            TAA_EFFECT.into(),
            names(&mut TAA_KNOBS.iter().map(|k| k.field)),
        )],
    }
}

const ROWS: usize = volume::FIELDS.len() + EFFECTS.len();

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

pub fn effect(name: &str) -> Option<&'static Effect> {
    EFFECTS.iter().find(|e| e.name == name)
}

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
