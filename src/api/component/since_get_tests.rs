//! `Component.Set` with a table held across ticks, and the second review's fixes
//! (#1369): unchanged means unchanged since `Get`, `graph = ""`, a renamed UI
//! shader, LOD levels, flattened Animator types.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::register;
use crate::components::UiShader;
use crate::scene::authoring::{add_component, ComponentKind};
use crate::scene::Scene;

/// Run each chunk in turn over entities `id` (every Add Component kind) and `other`
/// (a Rigidbody), with `tick` run on the scene between two chunks, as a frame
/// would; the last chunk's value.
fn run<R: mlua::FromLuaMulti>(chunks: &[&str], mut tick: impl FnMut(&mut Scene)) -> (Scene, R) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Dressed".to_string());
    for add in ComponentKind::ALL {
        add_component(&mut scene, id, *add);
    }
    let other = scene.add_entity("Other".to_string());
    add_component(&mut scene, other, ComponentKind::RigidBody);
    let scene = RefCell::new(scene);
    let lua = Lua::new();
    lua.globals().set("id", id).unwrap();
    lua.globals().set("other", other).unwrap();
    let out = lua.scope(|s| {
        register(&lua, s, &scene).unwrap();
        let (last, first) = chunks.split_last().unwrap();
        for chunk in first {
            lua.load(*chunk).exec()?;
            tick(&mut scene.borrow_mut());
        }
        lua.load(*last).eval::<R>()
    });
    (scene.into_inner(), out.unwrap_or_else(|e| panic!("{e}")))
}

#[test]
fn a_table_held_across_a_tick_writes_only_its_edits() {
    let (scene, ()) = run(
        &[
            r#"rb = Component.Get(id, "RigidBody"); a = Component.Get(id, "Animator")"#,
            r#"rb.mass = 5; Component.Set(id, "RigidBody", rb)
               a.speed = 2; Component.Set(id, "Animator", a)"#,
        ],
        |scene| {
            scene.world.rigidbody_mut(1).unwrap().velocity = Vec3::new(0.0, -3.0, 0.0);
            scene.world.animator_mut(1).unwrap().base.time = 0.75;
        },
    );
    let rb = scene.world.rigidbody(1).unwrap();
    assert_eq!(
        (rb.mass, rb.velocity.y),
        (5.0, -3.0),
        "the sim's velocity stays"
    );
    let a = scene.world.animator(1).unwrap();
    assert_eq!(
        (a.speed, a.base.time),
        (2.0, 0.75),
        "time ticked, not an error"
    );
}

#[test]
fn a_table_not_from_get_on_this_entity_writes_every_change() {
    let (scene, ()) = run(
        &[
            r#"Component.Set(other, "RigidBody", { mass = 7 })"#,
            r#"Component.Set(id, "RigidBody", { velocity = { 0, 0, 0 } })
               Component.Set(id, "RigidBody", Component.Get(other, "RigidBody"))"#,
        ],
        |scene| scene.world.rigidbody_mut(1).unwrap().velocity = Vec3::Y,
    );
    let rb = scene.world.rigidbody(1).unwrap();
    assert_eq!((rb.mass, rb.velocity), (7.0, Vec3::ZERO));
}

#[test]
fn an_empty_graph_is_no_graph() {
    let (scene, ()) = run(
        &[r#"Component.Set(id, "Animator", { graph = "rig.graph" })
             Component.Set(id, "Animator", { graph = "" })"#],
        |_| {},
    );
    assert_eq!(scene.world.animator(1).unwrap().graph, None);
}

#[test]
fn renaming_a_ui_shader_drops_the_old_params() {
    let (scene, ()) = run(
        &[
            "",
            r#"local i = Component.Get(id, "Image")
               i.shader.name = "glow"
               Component.Set(id, "Image", i)"#,
        ],
        |scene| {
            let params = [("tint".to_string(), vec![1.0, 0.0, 0.0, 1.0])].into();
            scene.world.image_mut(1).unwrap().shader = Some(UiShader {
                name: "wave".to_string(),
                params,
            });
        },
    );
    let shader = scene.world.image(1).unwrap().shader.clone().unwrap();
    assert_eq!(shader.name, "glow");
    assert!(shader.params.is_empty(), "{:?}", shader.params);
}

#[test]
fn lod_levels_take_any_order_of_old_thresholds() {
    let (scene, ()) = run(
        &[r#"Component.Set(id, "LODGroup", { levels = {
                { screen_height = 0.9, renderers = {} },
                { screen_height = 0.8, renderers = { 4 } },
                { screen_height = 0.7, renderers = {} },
                { screen_height = 0.75, renderers = {} } } })"#],
        |_| {},
    );
    let heights: Vec<f32> = scene
        .world
        .lod_group(1)
        .unwrap()
        .levels
        .iter()
        .map(|l| l.screen_height)
        .collect();
    assert_eq!(
        heights,
        [0.9, 0.8, 0.7, 0.7],
        "the last clamped under its finer one"
    );
}

#[test]
fn fields_types_the_animators_playback() {
    let (_, out) = run::<String>(
        &[r#"for _, row in ipairs(Component.Fields("Animator")) do
                if row.name == "previous_clip" then return row.type end
             end"#],
        |_| {},
    );
    assert_eq!(out, "optional string");
}
