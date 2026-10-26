//! `Graphics.{Get,Set}Vignette*`, `{Get,Set}ChromaticAberration*` and
//! `{Get,Set}FilmGrain*` (#1439): the lens effects on the active post-processing
//! volume, through the same `authoring::rendering::post_processing` ops the
//! Inspector card calls.

use std::cell::RefCell;

use super::super::{put, Reg};
use super::state::{with_vc, with_vc_mut};
use crate::components::{ChromaticAberration, FilmGrain, PostProcessingComponent, Vignette};
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::Scene;

/// One scalar knob: `Set<name>(value)` / `Get<name>()` through its op, and the
/// value the getter reports without an active volume (what a new volume starts
/// with; `false` for an `active` flag).
struct Knob<T> {
    name: &'static str,
    set: fn(&mut PostProcessingComponent, T),
    get: fn(&PostProcessingComponent) -> T,
    default: T,
}

/// The `active` flags.
const FLAGS: &[Knob<bool>] = &[
    Knob {
        name: "VignetteActive",
        set: vc_ops::set_vignette_active,
        get: |vc| vc.vignette.active,
        default: false,
    },
    Knob {
        name: "ChromaticAberrationActive",
        set: vc_ops::set_chromatic_aberration_active,
        get: |vc| vc.chromatic_aberration.active,
        default: false,
    },
    Knob {
        name: "FilmGrainActive",
        set: vc_ops::set_film_grain_active,
        get: |vc| vc.film_grain.active,
        default: false,
    },
];

/// The scalar parameters.
const NUMBERS: &[Knob<f32>] = &[
    Knob {
        name: "VignetteIntensity",
        set: vc_ops::set_vignette_intensity,
        get: |vc| vc.vignette.intensity,
        default: VIGNETTE.intensity,
    },
    Knob {
        name: "VignetteSmoothness",
        set: vc_ops::set_vignette_smoothness,
        get: |vc| vc.vignette.smoothness,
        default: VIGNETTE.smoothness,
    },
    Knob {
        name: "ChromaticAberrationIntensity",
        set: vc_ops::set_chromatic_aberration_intensity,
        get: |vc| vc.chromatic_aberration.intensity,
        default: ChromaticAberration::DEFAULT.intensity,
    },
    Knob {
        name: "FilmGrainIntensity",
        set: vc_ops::set_film_grain_intensity,
        get: |vc| vc.film_grain.intensity,
        default: FilmGrain::DEFAULT.intensity,
    },
    Knob {
        name: "FilmGrainResponse",
        set: vc_ops::set_film_grain_response,
        get: |vc| vc.film_grain.response,
        default: FilmGrain::DEFAULT.response,
    },
];

const VIGNETTE: Vignette = Vignette::DEFAULT;

/// Register one knob's setter and getter.
fn register_knob<'scope, T>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    knob: &'static Knob<T>,
) -> Reg
where
    T: Copy + mlua::FromLuaMulti + mlua::IntoLuaMulti + 'static,
{
    put(
        table,
        &format!("Set{}", knob.name),
        scope.create_function(move |_, v: T| {
            with_vc_mut(scene, |e| (knob.set)(e, v));
            Ok(())
        }),
    )?;
    put(
        table,
        &format!("Get{}", knob.name),
        scope.create_function(move |_, ()| Ok(with_vc(scene, knob.get).unwrap_or(knob.default))),
    )
}

/// The three effects' knobs.
pub(super) fn register_lens<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    for knob in FLAGS {
        register_knob(scope, table, scene, knob)?;
    }
    for knob in NUMBERS {
        register_knob(scope, table, scene, knob)?;
    }
    register_vignette_vectors(scope, table, scene)
}

/// `SetVignetteColor(r, g, b)` / `GetVignetteColor() -> r, g, b` and
/// `SetVignetteCenter(x, y)` / `GetVignetteCenter() -> x, y`.
fn register_vignette_vectors<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let defaults = VIGNETTE;
    put(
        table,
        "SetVignetteColor",
        scope.create_function(|_, (r, g, b): (f32, f32, f32)| {
            with_vc_mut(scene, |e| vc_ops::set_vignette_color(e, [r, g, b]));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetVignetteColor",
        scope.create_function(move |_, ()| {
            let [r, g, b] = with_vc(scene, |vc| vc.vignette.color).unwrap_or(defaults.color);
            Ok((r, g, b))
        }),
    )?;
    put(
        table,
        "SetVignetteCenter",
        scope.create_function(|_, (x, y): (f32, f32)| {
            with_vc_mut(scene, |e| vc_ops::set_vignette_center(e, [x, y]));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetVignetteCenter",
        scope.create_function(move |_, ()| {
            let [x, y] = with_vc(scene, |vc| vc.vignette.center).unwrap_or(defaults.center);
            Ok((x, y))
        }),
    )
}
