//! The engine's Lua-written verbs: `Particles`' Rust halves are timed as the
//! verbs they serve, and the script's `fn` and the Lua loop are not.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty_components::particle::{EmitMode, ParticleEmitterComponent};
use rusty_scene::Scene;
use rusty_script::api::{bind, particle};

use super::super::verb;
use crate::stats::verbs::take;
use crate::stats::verbs::wrap::wrap_globals;

#[test]
fn particles_verbs_have_rows_and_their_fn_is_not_one() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Sparks".to_string());
    let emitter = ParticleEmitterComponent {
        emit_mode: EmitMode::Burst,
        burst_count: 8,
        looping: false,
        max_particles: 8,
        ..Default::default()
    };
    scene.world.set_particles(id, Some(emitter));
    let scene = Rc::new(RefCell::new(scene));
    let lua = Lua::new();
    particle::register(&lua, bind(&lua, &scene)).unwrap();
    wrap_globals(&lua).unwrap();
    take();
    let code = format!(
        "Particles.Burst({id})
         local seen = 0
         Particles.ForEach({id}, function(p) seen = seen + 1 p.vy = 2 end)
         Particles.SetParticles({id}, Particles.GetParticles({id}))
         return seen"
    );
    let seen: u32 = lua.load(code).eval().unwrap();
    assert_eq!(seen, 8, "ForEach still visits every particle");
    let frame = take();
    let calls = |name| verb(&frame.verbs, name).map(|v| v.calls);
    // ForEach: its read and its edit, one row.
    assert_eq!(calls("Particles.ForEach"), Some(2), "{:?}", frame.verbs);
    assert_eq!(calls("Particles.GetParticles"), Some(1));
    assert_eq!(calls("Particles.SetParticles"), Some(1));
    let v: f64 = lua
        .load(format!("return Particles.GetParticles({id})[1].vy"))
        .eval()
        .unwrap();
    assert_eq!(v, 2.0, "the edits went back through the timed half");
}
