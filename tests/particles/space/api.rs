//! `Particles.SetSimulationSpace` / `GetSimulationSpace`, a scripted burst after
//! the emitter moved, a sub-emitter fired from a local particle, and the field
//! surviving save / load.

use std::cell::RefCell;

use glam::{Quat, Vec3};
use mlua::Lua;
use rusty::components::particle::{EmitMode, ParticleEmitterComponent, SimulationSpace};
use rusty::core::curve::Range;
use rusty::scene::Scene;

use super::{burst, emitter, state};

/// Run `code` against `scene` with the `Particles` namespace registered.
fn lua<R: mlua::FromLuaMulti>(scene: &RefCell<Scene>, code: &str) -> mlua::Result<R> {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::particle::register(&lua, scope, scene).unwrap();
        lua.load(code).eval::<R>()
    })
}

#[test]
fn set_and_get_round_trip_and_a_bad_name_is_refused() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Emitter".to_string());
    scene
        .world
        .set_particles(id, Some(ParticleEmitterComponent::default()));
    let scene = RefCell::new(scene);
    let get = format!("return Particles.GetSimulationSpace({id})");
    assert_eq!(lua::<String>(&scene, &get).unwrap(), "World");
    lua::<()>(
        &scene,
        &format!("Particles.SetSimulationSpace({id}, 'local')"),
    )
    .unwrap();
    assert_eq!(lua::<String>(&scene, &get).unwrap(), "Local");
    let bad = lua::<()>(
        &scene,
        &format!("Particles.SetSimulationSpace({id}, 'Custom')"),
    );
    let err = bad.unwrap_err().to_string();
    assert!(err.contains("unknown simulation space"), "{err}");
    assert_eq!(lua::<String>(&scene, &get).unwrap(), "Local", "unchanged");
    let bare = scene.borrow_mut().add_entity("Bare".to_string());
    let none = format!("return Particles.GetSimulationSpace({bare})");
    assert_eq!(lua::<Option<String>>(&scene, &none).unwrap(), None);
}

#[test]
fn a_burst_after_a_move_is_born_on_the_emitter_and_carried_once() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Emitter".to_string());
    let mut cfg = emitter(SimulationSpace::Local, 0.0);
    cfg.shape = rusty::components::particle::EmitShape::Point;
    scene.world.set_particles(id, Some(cfg));
    let scene = RefCell::new(scene);
    lua::<()>(&scene, &format!("Particles.Burst({id})")).unwrap();
    let to = Vec3::new(0.0, 3.0, -2.0);
    scene.borrow_mut().world.transform_mut(id).unwrap().position = to;
    lua::<()>(&scene, &format!("Particles.Burst({id})")).unwrap();
    let mut s = scene.borrow_mut();
    let frame = s.world.local_to_world(id);
    let mut e = s.world.particles_mut(id).unwrap();
    e.carry_to(frame);
    assert_eq!(e.live_count(), 16);
    for p in &e.runtime.particles {
        assert!(p.position.abs_diff_eq(to, 1e-5), "{}", p.position);
    }
}

#[test]
fn a_sub_emitter_from_a_local_particle_spawns_at_its_world_position() {
    let (world, smoke) = {
        let (mut world, id) = burst(SimulationSpace::Local, 0.0);
        let mut scene = world.scene().borrow_mut();
        let smoke = scene.add_entity("Smoke".to_string());
        let child = ParticleEmitterComponent {
            active: false,
            burst_count: 1,
            emit_mode: EmitMode::Continuous,
            lifetime: Range::constant(100.0),
            speed: Range::constant(0.0),
            spread: 0.0,
            ..Default::default()
        };
        scene.world.set_particles(smoke, Some(child));
        let mut t = scene.world.transform_mut(id).unwrap();
        t.position = Vec3::new(4.0, 1.0, 0.0);
        t.rotation = Quat::from_rotation_z(1.0);
        drop(t);
        let mut e = scene.world.particles_mut(id).unwrap();
        e.sub_emitters.death = Some(smoke);
        e.lifetime = Range::constant(0.0);
        e.runtime
            .particles
            .iter_mut()
            .for_each(|p| p.lifetime = 0.0);
        drop(e);
        drop(scene);
        world.tick(1.0 / 60.0);
        (world, smoke)
    };
    let puffs = state(&world, smoke);
    assert_eq!(puffs.len(), 8, "one puff per dead spark");
    for (p, _) in &puffs {
        assert!(
            p.distance(Vec3::new(4.0, 1.0, 0.0)) < 1.0,
            "{p} is on the moved emitter"
        );
    }
}

#[test]
fn the_field_survives_save_and_load() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Flash".to_string());
    let cfg = emitter(SimulationSpace::Local, 1.0);
    scene.world.set_particles(id, Some(cfg));
    let path = crate::temp::dir().join("rusty_particle_space.scene");
    let path = path.to_string_lossy().into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    let e = loaded.world.entity_document(id).unwrap();
    let space = e.particles.as_ref().unwrap().simulation_space;
    assert_eq!(space, SimulationSpace::Local);
}
