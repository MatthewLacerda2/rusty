use std::collections::BTreeMap;

use serde::Deserialize;

use super::Effect;
use crate::authoring::fields::table::{unheld, Access, Field};
use crate::authoring::fields::value::decode;
use rusty_components::PostProcessingComponent;
use rusty_scene::rules::sanitize::finite_values;
use rusty_shadergen::post_params::PostChain;

/// A name that reaches outside the shader workspace is refused; a module not baked yet is accepted,
/// and the renderer skips it until it is.
pub fn set_names(vc: &mut PostProcessingComponent, names: Vec<String>) -> Result<(), String> {
    if let Some(bad) = names.iter().find(|n| !is_module_name(n)) {
        return Err(format!(
            "invalid post-FX effect name {bad:?}: want a baked module name, not a path"
        ));
    }
    vc.custom_effects.names = names;
    Ok(())
}

/// A buffer write for the renderer, never a re-bake.
pub fn set_param(
    vc: &mut PostProcessingComponent,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    set_param_with(vc, &chain(vc), name, value)
}

/// Both `_with`s take a [`chain`] resolved once, for a tween, not the sidecars read each tick.
pub fn set_param_with(
    vc: &mut PostProcessingComponent,
    chain: &PostChain,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    chain.set(&mut vc.custom_effects.params, name, finite_values(value)?)
}

pub fn param(vc: &PostProcessingComponent, name: &str) -> Result<Vec<f32>, String> {
    param_with(vc, &chain(vc), name)
}

pub fn param_with(
    vc: &PostProcessingComponent,
    chain: &PostChain,
    name: &str,
) -> Result<Vec<f32>, String> {
    chain.get(&vc.custom_effects.params, name)
}

pub fn chain(vc: &PostProcessingComponent) -> PostChain {
    PostChain::load(&vc.custom_effects.names)
}

fn is_module_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\'])
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    names: Option<Vec<String>>,
    #[serde(default)]
    params: BTreeMap<String, Vec<f32>>,
}

/// A param handed back unchanged is not checked again, so a `Get` table with a new list writes no
/// stale param.
fn write(vc: &mut PostProcessingComponent, json: serde_json::Value) -> Result<(), String> {
    let written: Written = decode(json)?;
    if let Some(names) = written.names {
        set_names(vc, names)?;
    }
    let chain = chain(vc);
    for (name, value) in written.params {
        if vc.custom_effects.params.get(&name) != Some(&value) {
            set_param_with(vc, &chain, &name, value)?;
        }
    }
    Ok(())
}

pub const EFFECT: Effect = Effect {
    name: "custom_effects",
    field: unheld!(
        "its op reads every effect's baked sidecar and warns on an unbaked one",
        Field {
            name: "custom_effects",
            access: Access::Merge(write),
            unheld: None,
        }
    ),
    knobs: &[],
};
