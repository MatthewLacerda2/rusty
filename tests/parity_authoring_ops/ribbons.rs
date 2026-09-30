//! `Trail.*` / `Line.*` bindings vs `authoring::{trail, line, ribbon}` (#441).

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::components::{LineComponent, ParticleBlend, TextureMode, TrailComponent};
use rusty::core::curve::{Curve, Gradient};
use rusty::scene::authoring::{line as line_ops, ribbon as ribbon_ops, trail as trail_ops};
use rusty::scene::Scene;

/// Two sibling entities (Lua-driven, op-driven), each given a default component
/// by `attach`; runs `script` (with `{id}` = the Lua entity) through both
/// namespaces. Returns the scene and the two ids.
fn drive(attach: fn(&mut Scene, u32) -> bool, script: &str) -> (RefCell<Scene>, u32, u32) {
    let scene = RefCell::new(Scene::new());
    let via_lua = scene.borrow_mut().add_entity("ViaLua".to_string());
    let via_op = scene.borrow_mut().add_entity("ViaOp".to_string());
    attach(&mut scene.borrow_mut(), via_lua);
    attach(&mut scene.borrow_mut(), via_op);
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::trail::register(&lua, s, &scene).unwrap();
        rusty::api::line::register(&lua, s, &scene).unwrap();
        let code = script.replace("{id}", &via_lua.to_string());
        lua.load(code).exec().unwrap();
        Ok(())
    })
    .unwrap();
    (scene, via_lua, via_op)
}

#[test]
fn trail_api_and_shared_ops_converge() {
    let (scene, via_lua, via_op) = drive(
        |s, id| s.world.set_trail(id, Some(TrailComponent::default())),
        r#"Trail.SetEmitting({id}, false)
           Trail.SetTime({id}, -1)
           Trail.SetMinVertexDistance({id}, 0.3)
           Trail.SetWidth({id}, 0.5, -2)
           Trail.SetColors({id}, 2, 0, 0, 1,  0, 0, 1, 3)
           Trail.SetBlend({id}, "Additive")"#,
    );
    {
        let mut s = scene.borrow_mut();
        let mut t = s.world.trail_mut(via_op).unwrap();
        trail_ops::set_emitting(&mut t, false);
        trail_ops::set_time(&mut t, -1.0);
        trail_ops::set_min_vertex_distance(&mut t, 0.3);
        ribbon_ops::set_width(&mut t.style, Curve::linear(0.5, -2.0));
        let colors = Gradient::linear([2.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 3.0]);
        ribbon_ops::set_color(&mut t.style, colors);
        ribbon_ops::set_blend(&mut t.style, ParticleBlend::Additive);
    }
    let s = scene.borrow();
    let trail = |id| s.world.trail(id).map(|t| t.clone()).unwrap();
    assert_eq!(trail(via_lua), trail(via_op));
    assert_eq!(trail(via_op).time, 0.0, "single-sourced clamp");
    assert_eq!(trail(via_op).style.color.evaluate(1.0)[3], 1.0, "alpha ≤ 1");
}

#[test]
fn line_api_and_shared_ops_converge() {
    let (scene, via_lua, via_op) = drive(
        |s, id| s.world.set_line(id, Some(LineComponent::default())),
        r#"Line.SetPositions({id}, {{0, 0, 0}, {1, 1, 1}})
           Line.SetPosition({id}, 1, 2, 2, 2)
           Line.SetUseWorldSpace({id}, false)
           Line.SetLoop({id}, true)
           Line.SetTexture({id}, "beam.png")
           Line.SetTextureMode({id}, "Tile")"#,
    );
    {
        let mut s = scene.borrow_mut();
        let mut l = s.world.line_mut(via_op).unwrap();
        line_ops::set_positions(&mut l, &[Vec3::ZERO, Vec3::ONE]);
        line_ops::set_position(&mut l, 1, Vec3::splat(2.0));
        line_ops::set_use_world_space(&mut l, false);
        line_ops::set_looping(&mut l, true);
        ribbon_ops::set_texture(&mut l.style, "beam.png".to_string());
        ribbon_ops::set_texture_mode(&mut l.style, TextureMode::Tile);
    }
    let s = scene.borrow();
    let line = |id| s.world.line(id).map(|l| l.clone());
    assert_eq!(line(via_lua), line(via_op));
}
