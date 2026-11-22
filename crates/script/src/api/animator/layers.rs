use std::path::Path;

use mlua::Value;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::ConsoleLogs;
use rusty_asset::animation_graph;
use rusty_components::AnimatorComponent;
use rusty_scene::Scene;

pub(super) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    register_layer_weight(lua, table, scene, console)?;
    register_get_current_node(lua, table, scene)
}

pub(super) fn lookup(anim: &AnimatorComponent, layer: &Value) -> Option<usize> {
    let index = match layer {
        Value::Integer(i) => usize::try_from(*i).ok()?,
        Value::Number(n) if n.fract() == 0.0 && *n >= 0.0 => *n as usize,
        Value::String(s) => return anim.layer_index(&s.to_str().ok()?),
        _ => return None,
    };
    (index <= anim.layers.len()).then_some(index)
}

pub(super) fn resolve(anim: &mut AnimatorComponent, layer: &Value) -> Option<usize> {
    if let Some(index) = lookup(anim, layer) {
        return Some(index);
    }
    let path = anim.graph.clone()?;
    let graph = animation_graph::load(Path::new(&path)).ok()?;
    anim.sync_layers(&graph);
    lookup(anim, layer)
}

fn register_layer_weight(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "SetLayerWeight",
        lua.create_function(move |lua, (id, layer, weight): (EntityArg, Value, f32)| {
            scene.with(lua, |scene| {
                console.with(lua, |console| {
                    let id = id.live("Animator", "SetLayerWeight")?;
                    let mut scene = scene.borrow_mut();
                    let Some(mut anim) = scene.world.animator_mut(id) else {
                        return Ok(false);
                    };
                    match resolve(&mut anim, &layer) {
                        Some(0) => console.borrow_mut().warn(
                            "Animator.SetLayerWeight: the base layer is always at full weight"
                                .to_string(),
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
                })
            })
        }),
    )?;

    put(
        table,
        "GetLayerWeight",
        lua.create_function(move |lua, (id, layer): (EntityArg, Value)| {
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let Some(mut anim) = scene.world.animator_mut(id.read()) else {
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

fn register_get_current_node(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let reader = |read: fn(&AnimatorComponent, usize, &str) -> Option<String>| {
        move |lua: &mlua::Lua, (id, layer): (EntityArg, Option<Value>)| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let Some(anim) = scene.world.animator(id.read()) else {
                return Ok(None);
            };
            let Some(index) = layer.map_or(Some(0), |l| lookup(&anim, &l)) else {
                return Ok(None);
            };
            let playback = match index {
                0 => &anim.base,
                i => &anim.layers[i - 1].playback,
            };
            Ok(playback
                .current_node
                .as_deref()
                .and_then(|node| read(&anim, index, node)))
        }
    };
    let node = reader(|_, _, node| Some(node.to_string()));
    put(table, "GetCurrentNode", lua.create_function(node))?;
    put(
        table,
        "GetCurrentTag",
        lua.create_function(reader(node_tag)),
    )
}

fn node_tag(anim: &AnimatorComponent, index: usize, node: &str) -> Option<String> {
    let graph = animation_graph::load(Path::new(anim.graph.as_deref()?)).ok()?;
    let tag = &graph.machine(index)?.node(node)?.tag;
    (!tag.is_empty()).then(|| tag.clone())
}

pub(super) fn describe(layer: &Value) -> String {
    match layer {
        Value::String(s) => format!("'{}'", s.to_string_lossy()),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.type_name().to_string(),
    }
}
