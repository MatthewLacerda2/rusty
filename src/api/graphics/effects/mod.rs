//! `Graphics.*` per post effect (#1467): one registrar per effect, in its own file
//! (grading and lens effects in their folders), listed once in [`REGISTRARS`]. A
//! volume effect writes the main camera's active post-processing volume and a camera
//! effect (motion blur, FXAA) the main camera's component, both through the
//! `scene::authoring` ops the Inspector calls; an effect's numeric knobs that are a
//! pair on their own come from its knob table ([`knobs`]).

use std::cell::RefCell;

use super::super::Reg;
use crate::scene::Scene;

// sorted: one module per effect (or family of effects); none reads another
mod bloom;
mod custom_effects;
mod depth_of_field;
#[cfg(test)]
mod dummy;
mod exposure;
mod fxaa;
mod grading;
mod knobs;
mod lens;
mod lens_flare;
mod motion_blur;
mod ssr;

use grading::{color_adjustments, color_lookup, lift_gamma_gain, tonemapping, white_balance};
use lens::{chromatic_aberration, film_grain, vignette};

/// One effect's `Graphics.*` functions, registered onto the namespace's table.
pub(crate) type Registrar =
    for<'scope> fn(&'scope mlua::Scope<'scope, '_>, &mlua::Table, &'scope RefCell<Scene>) -> Reg;

/// Every effect's registrar, by the effect's name.
pub(crate) const REGISTRARS: &[(&str, Registrar)] = &[
    // sorted: one row per effect; each registers names no other does
    ("bloom", bloom::register),
    ("chromatic_aberration", chromatic_aberration::register),
    ("color_adjustments", color_adjustments::register),
    ("color_lookup", color_lookup::register),
    ("custom_effects", custom_effects::register),
    ("depth_of_field", depth_of_field::register),
    #[cfg(test)]
    ("dummy", dummy::register),
    ("exposure", exposure::register),
    ("film_grain", film_grain::register),
    ("fxaa", fxaa::register),
    ("lens_flare", lens_flare::register),
    ("lift_gamma_gain", lift_gamma_gain::register),
    ("motion_blur", motion_blur::register),
    ("ssr", ssr::register),
    ("tonemapping", tonemapping::register),
    ("vignette", vignette::register),
    ("white_balance", white_balance::register),
];

/// Register every effect's functions onto `table`.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    for (_, register) in REGISTRARS {
        register(scope, table, scene)?;
    }
    Ok(())
}
