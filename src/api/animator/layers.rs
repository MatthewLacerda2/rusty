//! src/api/animator/layers.rs — the `Animator` layer surface (#457).
//!
//! `SetLayerWeight` / `GetLayerWeight` and the layer-aware `GetCurrentNode`. A
//! layer is addressed Unity's way, by index — 0 is the base layer, 1.. the
//! graph's extra layers in order — or, rusty's convenience, by its name in the
//! graph. A script's `Start` runs before the evaluator's first step, so a layer
//! that isn't bound yet is bound here from the graph asset on first use rather
//! than refused.

use std::cell::RefCell;
use std::path::Path;

use mlua::Value;

use super::{put, Reg};
use crate::asset::animation_graph;
use crate::components::AnimatorComponent;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the layer functions onto the `Animator` table.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    register_layer_weight(scope, table, scene, console)?;
    register_get_current_node(scope, table, scene)
}

/// The layer `layer` names on `anim` (0 = base), among the layers bound so far.
pub(super) fn lookup(anim: &AnimatorComponent, layer: &Value) -> Option<usize> {
    let index = match layer {
        Value::Integer(i) => usize::try_from(*i).ok()?,
        Value::Number(n) if n.fract() == 0.0 && *n >= 0.0 => *n as usize,
        Value::String(s) => return anim.layer_index(&s.to_str().ok()?),
        _ => return None,
    };
    (index <= anim.layers.len()).then_some(index)
}

/// [`lookup`], binding the graph's layers from its asset first when the layer
/// isn't found among the bound ones (the evaluator hasn't stepped yet).
pub(super) fn resolve(anim: &mut AnimatorComponent, layer: &Value) -> Option<usize> {
    if let Some(index) = lookup(anim, layer) {
        return Some(index);
    }
    let path = anim.graph.clone()?;
    let graph = animation_graph::load(Path::new(&path)).ok()?;
    anim.sync_layers(&graph);
    lookup(anim, layer)
}

/// `SetLayerWeight(id, layer, w)` — set an extra layer's weight, clamped to
/// `[0, 1]`; `true` when it was set. The base layer is always at full weight
/// (as in Unity), so layer 0 is refused with a warning, as is an unknown layer.
/// `GetLayerWeight(id, layer)` — the live weight (`1` for the base layer), or
/// `nil` for an unknown layer.
fn register_layer_weight<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "SetLayerWeight",
        scope.create_function(|_, (id, layer, weight): (u32, Value, f32)| {
            let mut scene = scene.borrow_mut();
            let Some(mut anim) = scene.world.animator_mut(id) else {
                return Ok(false);
            };
            match resolve(&mut anim, &layer) {
                Some(0) => console.borrow_mut().warn(
                    "Animator.SetLayerWeight: the base layer is always at full weight".to_string(),
                ),
                Some(index) => {
                    anim.layers[index - 1].weight = weight.clamp(0.0, 1.0);
                    return Ok(true);
                }
                None => console.borrow_mut().warn(format!(
                    "Animator.SetLayerWeight: entity {id} has no layer {}",
                    describe(&layer)
                )),
            }
            Ok(false)
        }),
    )?;

    put(
        table,
        "GetLayerWeight",
        scope.create_function(|_, (id, layer): (u32, Value)| {
            let mut scene = scene.borrow_mut();
            let Some(mut anim) = scene.world.animator_mut(id) else {
                return Ok(None);
            };
            Ok(match resolve(&mut anim, &layer) {
                Some(0) => Some(1.0),
                Some(index) => Some(anim.layers[index - 1].weight),
                None => None,
            })
        }),
    )
}

/// `GetCurrentNode(id [, layer])` — the active graph node's name in `layer`
/// (default: the base layer), or `nil` when there is no animator, no such layer,
/// or the evaluator hasn't bound it yet.
fn register_get_current_node<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetCurrentNode",
        scope.create_function(|_, (id, layer): (u32, Option<Value>)| {
            let scene = scene.borrow();
            let Some(anim) = scene.world.animator(id) else {
                return Ok(None);
            };
            let playback = match layer.map_or(Some(0), |l| lookup(&anim, &l)) {
                Some(0) => &anim.base,
                Some(index) => &anim.layers[index - 1].playback,
                None => return Ok(None),
            };
            Ok(playback.current_node.clone())
        }),
    )
}

/// A layer argument as a warning shows it.
pub(super) fn describe(layer: &Value) -> String {
    match layer {
        Value::String(s) => format!("'{}'", s.to_string_lossy()),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.type_name().to_string(),
    }
}
