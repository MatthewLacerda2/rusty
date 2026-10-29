//! The `Graphics.Get/Set<api>` pair of every effect knob that is one on its own
//! (#1467): each effect's knob table (`authoring::rendering::post_processing`)
//! names the pair, so the functions, the Inspector's sliders and the
//! `Graphics.<effect>.<knob>` tween paths reach the same value through one op.

use std::cell::RefCell;

use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use crate::components::PostProcessingComponent;
use crate::core::settings::quality::EffectQuality;
use crate::scene::authoring::default_post_processing;
use crate::scene::authoring::rendering::post_processing::Knob;
use crate::scene::Scene;

/// Register `Set<api>(value)` / `Get<api>()` for each of `knobs` that names an
/// `api`. Without an active volume the setter does nothing and the getter reports
/// what a new volume starts with.
pub(in crate::api::graphics) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    knobs: &'static [Knob],
) -> Reg {
    for knob in knobs {
        let Some(api) = knob.api else { continue };
        put(
            table,
            &format!("Set{api}"),
            scope.create_function(move |_, value: f32| {
                with_vc_mut(scene, |vc| (knob.set)(vc, value));
                Ok(())
            }),
        )?;
        put(
            table,
            &format!("Get{api}"),
            scope.create_function(move |_, ()| {
                let value = with_vc(scene, knob.get);
                Ok(value.unwrap_or_else(|| (knob.get)(&default_post_processing())))
            }),
        )?;
    }
    Ok(())
}

/// Register `Set<name>(bool)` / `Get<name>()` over an effect's switch: `false`
/// without an active volume, since nothing runs then.
pub(in crate::api::graphics) fn register_flag<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    (table, scene): (&mlua::Table, &'scope RefCell<Scene>),
    name: &str,
    get: fn(&PostProcessingComponent) -> bool,
    set: fn(&mut PostProcessingComponent, bool),
) -> Reg {
    put(
        table,
        &format!("Set{name}"),
        scope.create_function(move |_, on: bool| {
            with_vc_mut(scene, |vc| set(vc, on));
            Ok(())
        }),
    )?;
    put(
        table,
        &format!("Get{name}"),
        scope.create_function(move |_, ()| Ok(with_vc(scene, get).unwrap_or(false))),
    )
}

/// `name` as a quality level, or `Graphics.<function>`'s refusal listing the levels.
pub(in crate::api::graphics) fn parse_quality(
    function: &str,
    name: &str,
) -> mlua::Result<EffectQuality> {
    EffectQuality::parse(name).ok_or_else(|| {
        mlua::Error::RuntimeError(format!(
            "Graphics.{function}: unknown quality '{name}' (one of {})",
            EffectQuality::names()
        ))
    })
}

/// Register `Set<name>(level)` / `Get<name>()` over an effect's quality level
/// (#1522): `"Low"`, `"Medium"`, `"High"` or `"Custom"`, an unknown one refused.
/// Without an active volume the getter reports a new volume's level.
pub(in crate::api::graphics) fn register_quality<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    (table, scene): (&mlua::Table, &'scope RefCell<Scene>),
    name: &'static str,
    get: fn(&PostProcessingComponent) -> EffectQuality,
    set: fn(&mut PostProcessingComponent, EffectQuality),
) -> Reg {
    put(
        table,
        &format!("Set{name}"),
        scope.create_function(move |_, level: String| {
            let quality = parse_quality(&format!("Set{name}"), &level)?;
            with_vc_mut(scene, |vc| set(vc, quality));
            Ok(())
        }),
    )?;
    put(
        table,
        &format!("Get{name}"),
        scope.create_function(move |_, ()| {
            let quality = with_vc(scene, get).unwrap_or_else(|| get(&default_post_processing()));
            Ok(quality.name())
        }),
    )
}
