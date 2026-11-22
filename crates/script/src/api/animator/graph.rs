use std::path::Path;

use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::ConsoleLogs;
use rusty_asset::animation_graph;
use rusty_edit::authoring::animator as animator_ops;
use rusty_scene::Scene;

use mlua::Value;

use super::{layers, put, Reg};

pub(super) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    register_set_graph(lua, table, scene)?;
    register_play_node(lua, table, scene, console)?;
    register_play_animation(lua, table, scene)
}

fn register_set_graph(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetGraph",
        lua.create_function(move |lua, (id, path): (EntityArg, String)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "SetGraph")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_graph(&mut c, (!path.is_empty()).then_some(path));
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "SetGraphEnabled",
        lua.create_function(move |lua, (id, enabled): (EntityArg, bool)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "SetGraphEnabled")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_graph_enabled(&mut c, enabled);
            }
            Ok(())
        }),
    )
}

fn register_play_node(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "PlayNode",
        lua.create_function(
            move |lua, (id, node, layer): (EntityArg, String, Option<Value>)| {
                let scene = &*scene.get(lua)?;
                let console = &*console.get(lua)?;
                let id = id.live("Animator", "PlayNode")?;
                let mut scene = scene.borrow_mut();
                let Some(mut anim) = scene.world.animator_mut(id) else {
                    return Ok(false);
                };
                let Some(path) = anim.graph.clone() else {
                    return Ok(false);
                };
                let warn = |message: String| {
                    console
                        .borrow_mut()
                        .warn(format!("Animator.PlayNode: graph '{path}'{message}"));
                    Ok(false)
                };
                let graph = match animation_graph::load(Path::new(&path)) {
                    Ok(graph) => graph,
                    Err(err) => return warn(format!(": {err}")),
                };
                anim.sync_layers(&graph);
                let index = match &layer {
                    None => 0,
                    Some(l) => match layers::lookup(&anim, l) {
                        Some(index) => index,
                        None => return warn(format!(" has no layer {}", layers::describe(l))),
                    },
                };
                let Some(target) = graph.machine(index).and_then(|m| m.node(&node)) else {
                    return warn(format!(" has no node '{node}'"));
                };
                match index {
                    0 => anim.enter_node(target, 0.0),
                    i => anim.layers[i - 1].playback.enter_node(target, 0.0),
                }
                Ok(true)
            },
        ),
    )
}

fn register_play_animation(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "PlayAnimation",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Animator", "PlayAnimation")?;
            let mut scene = scene.borrow_mut();
            let exists = scene
                .world
                .mesh(id)
                .is_some_and(|m| m.clips.iter().any(|c| c.name == name));
            if !exists {
                return Ok(false);
            }
            let Some(mut anim) = scene.world.animator_mut(id) else {
                return Ok(false);
            };
            anim.play(name);
            Ok(true)
        }),
    )
}
