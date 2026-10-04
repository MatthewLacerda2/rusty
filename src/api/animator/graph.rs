//! src/api/animator/graph.rs — the `Animator` graph control surface (#316).
//!
//! The script-facing overrides of graph evaluation: `SetGraph` assigns/clears the
//! `AnimationGraph` asset driving the animator (through the shared authoring op the
//! editor card also uses), `SetGraphEnabled` pauses/resumes auto-evaluation
//! (disabled, the animator stays under direct `Play`/`Crossfade` control),
//! `PlayNode` jumps straight to a named graph state bypassing conditions —
//! distinct from the raw clip-level `Play` — `PlayAnimation` plays a clip by name
//! and honestly reports whether the mesh carries it. Reading the active state
//! back (`GetCurrentNode`) lives with the layer surface in `layers.rs`.

use std::cell::RefCell;
use std::path::Path;

use crate::asset::animation_graph;
use crate::scene::authoring::animator as animator_ops;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

use mlua::Value;

use super::{layers, put, Reg};

/// Register the graph-control functions onto the `Animator` table.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    register_set_graph(scope, table, scene)?;
    register_play_node(scope, table, scene, console)?;
    register_play_animation(scope, table, scene)
}

/// `SetGraph` / `SetGraphEnabled` — assign (or clear, with `""`) the graph asset
/// path, and toggle auto-evaluation. Both route through the shared authoring ops.
fn register_set_graph<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetGraph",
        scope.create_function(|_, (id, path): (u32, String)| {
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
        scope.create_function(|_, (id, enabled): (u32, bool)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_graph_enabled(&mut c, enabled);
            }
            Ok(())
        }),
    )
}

/// `PlayNode(id, node [, layer])` — jump straight to the named graph node
/// (state) of `layer` (default: the base layer), bypassing every condition: a
/// hard cut into the node's motion, adopting its loop flag and speed. `true` when
/// the jump happened; `false` (with a console warning for a bad graph, unknown
/// layer or unknown node) otherwise.
fn register_play_node<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "PlayNode",
        scope.create_function(|_, (id, node, layer): (u32, String, Option<Value>)| {
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
        }),
    )
}

/// `PlayAnimation` — play a clip by name directly, returning whether the entity's
/// mesh actually carries a clip of that name (the honest, queryable sibling of the
/// silently no-op'ing `Play`).
fn register_play_animation<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "PlayAnimation",
        scope.create_function(|_, (id, name): (u32, String)| {
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
