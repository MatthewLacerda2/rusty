//! `Particles.GetParticles` / `SetParticles`: a script reads and rewrites
//! an emitter's live particles. This file and `edges` drive the pair over a bare
//! scene; `play` runs it inside the real play loop.

mod arrays;
mod bench;
mod edges;
mod for_each;
mod identity;
mod play;
mod stale;
mod types;

use std::cell::RefCell;
use std::rc::Rc;

use glam::{Quat, Vec3};
use mlua::Lua;
use rusty::components::particle::{EmitMode, EmitShape, ParticleEmitterComponent};
use rusty::core::curve::{ColorRange, Range};
use rusty::scene::Scene;
use rusty::script::api::bind;

/// Run `code` against `scene` with the `Particles` namespace registered.
fn lua<R: mlua::FromLuaMulti>(scene: &Rc<RefCell<Scene>>, code: &str) -> mlua::Result<R> {
    let lua = Lua::new();
    crate::lua::attempt(|| {
        rusty::script::api::particle::register(&lua, bind(&lua, scene)).unwrap();
        lua.load(code).eval::<R>()
    })
}

/// A long-lived burst emitter of `n` particles with every start range spread.
fn emitter(n: u32) -> ParticleEmitterComponent {
    ParticleEmitterComponent {
        emit_mode: EmitMode::Burst,
        burst_count: n,
        looping: false,
        shape: EmitShape::Sphere { radius: 1.0 },
        lifetime: Range::new(50.0, 100.0),
        speed: Range::new(1.0, 3.0),
        size: Range::new(0.1, 0.5),
        rotation: Range::new(-180.0, 180.0),
        color: ColorRange {
            min: [0.2, 0.3, 0.4, 0.5],
            max: [1.0, 0.9, 0.8, 1.0],
        },
        max_particles: 8,
        ..Default::default()
    }
}

/// A scene holding one emitter that has burst `n` particles.
fn scene(n: u32) -> (Rc<RefCell<Scene>>, u32) {
    scene_with(emitter(n))
}

/// A scene holding the emitter `cfg`, burst once.
fn scene_with(cfg: ParticleEmitterComponent) -> (Rc<RefCell<Scene>>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Sparks".to_string());
    scene.world.set_particles(id, Some(cfg));
    let scene = Rc::new(RefCell::new(scene));
    lua::<()>(&scene, &format!("Particles.Burst({id})")).unwrap();
    (scene, id)
}

/// Every live particle, printed exactly (`{:?}` round-trips an `f32`).
fn dump(scene: &Rc<RefCell<Scene>>, id: u32) -> String {
    format!(
        "{:?}",
        scene
            .borrow()
            .world
            .particles(id)
            .unwrap()
            .runtime
            .particles
    )
}

#[test]
fn a_round_trip_with_no_change_leaves_the_emitter_bit_identical() {
    let (scene, id) = scene(4);
    {
        let mut s = scene.borrow_mut();
        let mut e = s.world.particles_mut(id).unwrap();
        let p = &mut e.runtime.particles[1];
        (p.start_frame, p.axis, p.turn) = (3, Vec3::Z, Quat::from_rotation_x(0.7));
        p.custom = [1.5, -2.0, 0.1, 7.0];
    }
    let before = dump(&scene, id);
    let live: u32 = lua(
        &scene,
        &format!("return Particles.SetParticles({id}, Particles.GetParticles({id}))"),
    )
    .unwrap();
    assert_eq!(live, 4);
    assert_eq!(dump(&scene, id), before);
}

#[test]
fn a_table_reads_in_world_space_and_degrees() {
    let (scene, id) = scene(1);
    let (x, deg, a, c1): (f32, f32, f32, f32) = lua(
        &scene,
        &format!(
            "local p = Particles.GetParticles({id})[1]
             return p.x, p.rotation, p.a, p.custom1"
        ),
    )
    .unwrap();
    let s = scene.borrow();
    let p = s.world.particles(id).unwrap().runtime.particles[0];
    assert_eq!((x, a, c1), (p.position.x, p.color[3], 0.0));
    assert_eq!(deg, p.rotation.to_degrees());
}
