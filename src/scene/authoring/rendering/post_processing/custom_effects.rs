//! The authored post-FX chain's ops (#397, #671): the ordered module names and
//! their runtime params. No knobs: a param is tweened as `Graphics.<block.param>`,
//! through the chain's layouts.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::Effect;
use crate::components::PostProcessingComponent;
use crate::scene::authoring::fields::table::{unheld, Access, Field};
use crate::scene::authoring::fields::value::decode;
use crate::scene::authoring::sanitize::finite_values;
use crate::shadergen::post_params::PostChain;

/// Replace the ordered list of authored postfx modules the volume runs (#397).
///
/// Each entry is a module name as `Shader.Bake` wrote it (`<name>.wgsl`, no
/// extension). A name that is empty or reaches outside the shader workspace (a path
/// separator, a leading `.`) is refused by name and nothing is written; a module
/// that simply isn't baked yet is accepted — the renderer skips it until it is.
pub fn set_names(vc: &mut PostProcessingComponent, names: Vec<String>) -> Result<(), String> {
    if let Some(bad) = names.iter().find(|n| !is_module_name(n)) {
        return Err(format!(
            "invalid post-FX effect name {bad:?}: want a baked module name, not a path"
        ));
    }
    vc.custom_effects.names = names;
    Ok(())
}

/// Set runtime param `name` of the volume's authored effects (#671), checked against
/// the layouts of the modules in `custom_effects.names`: a name none of them exposes
/// at runtime, or a wrong number of values, is an error naming it and nothing is
/// written. One number broadcasts to every lane. A buffer write for the renderer,
/// never a re-bake.
pub fn set_param(
    vc: &mut PostProcessingComponent,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    set_param_with(vc, &chain(vc), name, value)
}

/// [`set_param`] against an already-read `chain` of the volume's effects — a tween
/// (#661) resolves it once at start instead of reading sidecars every tick.
pub fn set_param_with(
    vc: &mut PostProcessingComponent,
    chain: &PostChain,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    chain.set(&mut vc.custom_effects.params, name, finite_values(value)?)
}

/// The value runtime param `name` of the volume's effects draws with: the stored one,
/// else the effect's baked default. Strict like [`set_param`].
pub fn param(vc: &PostProcessingComponent, name: &str) -> Result<Vec<f32>, String> {
    param_with(vc, &chain(vc), name)
}

/// [`param`] against an already-read `chain`.
pub fn param_with(
    vc: &PostProcessingComponent,
    chain: &PostChain,
    name: &str,
) -> Result<Vec<f32>, String> {
    chain.get(&vc.custom_effects.params, name)
}

/// The layouts of the volume's effects, read from their baked sidecars.
pub fn chain(vc: &PostProcessingComponent) -> PostChain {
    PostChain::load(&vc.custom_effects.names)
}

/// A bare module file stem: non-empty, no path separator, not hidden or `..`.
fn is_module_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\'])
}

/// What a write of `custom_effects` names: the list, and the params it gives.
#[derive(Deserialize)]
struct Written {
    names: Option<Vec<String>>,
    #[serde(default)]
    params: BTreeMap<String, Vec<f32>>,
}

/// The list when given, then each given param that differs from the held one,
/// checked against the (new) list's layouts. A param handed back unchanged is not
/// checked again, so a `Get` table with a new list writes no stale param.
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
            access: Access::Op(write),
            unheld: None,
        }
    ),
    knobs: &[],
};
