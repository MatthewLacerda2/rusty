use super::super::Reg;
use crate::api::ApiCell;
use rusty_scene::Scene;

// sorted: one module per effect (or family of effects); none reads another
mod bloom;
mod custom_effects;
mod depth_of_field;
mod exposure;
mod fxaa;
mod grading;
pub(super) mod knobs;
mod lens;
mod lens_flare;
mod motion_blur;
mod ssil;
mod ssr;
mod taa;
mod volumetric_fog;

use grading::{color_adjustments, color_lookup, lift_gamma_gain, tonemapping, white_balance};
use lens::{chromatic_aberration, film_grain, vignette};

pub type Registrar = fn(&mlua::Lua, &mlua::Table, ApiCell<Scene>) -> Reg;

pub const REGISTRARS: &[(&str, Registrar)] = &[
    // sorted: one row per effect; each registers names no other does
    ("bloom", bloom::register),
    ("chromatic_aberration", chromatic_aberration::register),
    ("color_adjustments", color_adjustments::register),
    ("color_lookup", color_lookup::register),
    ("custom_effects", custom_effects::register),
    ("depth_of_field", depth_of_field::register),
    ("exposure", exposure::register),
    ("film_grain", film_grain::register),
    ("fxaa", fxaa::register),
    ("lens_flare", lens_flare::register),
    ("lift_gamma_gain", lift_gamma_gain::register),
    ("motion_blur", motion_blur::register),
    ("ssil", ssil::register),
    ("ssr", ssr::register),
    ("taa", taa::register),
    ("tonemapping", tonemapping::register),
    ("vignette", vignette::register),
    ("volumetric_fog", volumetric_fog::register),
    ("white_balance", white_balance::register),
];

pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    for (_, register) in REGISTRARS {
        register(lua, table, scene)?;
    }
    Ok(())
}
