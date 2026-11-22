use super::*;
use crate::authoring::fields::{find, Skip};
use crate::authoring::particles::{
    set_collision, set_max_particles, set_render_mode, set_simulation_space, set_sub_emitter,
};
use crate::authoring::{add_component, ComponentKind};
use rusty_components::particle::EmitterFrame;
use rusty_test_support::logs;
use serde_json::{json, Map, Value};

#[test]
fn a_switch_to_gpu_warns_for_each_setting_it_does_not_honour() {
    logs::init();
    let mut p = ParticleEmitterComponent {
        collision: CollisionResponse::Bounce,
        ..Default::default()
    };
    p.sub_emitters.death = Some(4);
    p.render.mode = ParticleRenderMode::Mesh;
    p.emit_at(EmitterFrame::IDENTITY, 1);
    assert!(
        gpu_warnings(&p).is_empty(),
        "a Cpu emitter honours them all"
    );
    set_simulation(&mut p, ParticleSimulation::Gpu);
    assert_eq!(p.live_count(), 0, "the switch forgets the live ones");
    let said = logs::take();
    assert_eq!(said.len(), 3, "{said:?}");
    assert!(said.iter().all(|s| s.contains("Gpu emitter")));
    set_simulation(&mut p, ParticleSimulation::Gpu);
    assert!(logs::take().is_empty(), "no change, no warning");
}

#[test]
fn each_op_on_a_gpu_emitter_warns_only_of_its_own_setting() {
    logs::init();
    let mut p = ParticleEmitterComponent::default();
    set_collision(&mut p, CollisionResponse::Die);
    assert!(logs::take().is_empty(), "a Cpu emitter honours it");
    set_simulation(&mut p, ParticleSimulation::Gpu);
    assert_eq!(logs::take().len(), 1, "the switch lists the collision");
    let one = |what: &str| {
        let said = logs::take();
        assert_eq!(said.len(), 1, "{what}: {said:?}");
        assert!(said[0].contains(what), "{what}: {said:?}");
    };
    set_render_mode(&mut p, ParticleRenderMode::Mesh);
    one("mesh");
    set_sub_emitter(&mut p, SubEmitTrigger::Death, Some(3));
    one("death");
    set_sub_emitter(&mut p, SubEmitTrigger::Collision, Some(3));
    one("collision sub-emitter");
    set_max_particles(&mut p, 2_000_000);
    one("max_particles");
    set_collision(&mut p, CollisionResponse::Bounce);
    one("Bounce");
    set_sub_emitter(&mut p, SubEmitTrigger::Birth, Some(3));
    set_max_particles(&mut p, 500);
    assert!(
        logs::take().is_empty(),
        "a birth sub-emitter fires; 500 fits"
    );
}

fn set(world: &mut rusty_ecs::World, id: u32, changes: Value) {
    let Value::Object(changes) = changes else {
        panic!("a table")
    };
    let kind = find("Particles").unwrap();
    assert_eq!((kind.write)(world, id, &changes, Skip::Held), Ok(true));
}

fn emitter() -> (rusty_scene::Scene, u32) {
    let mut scene = rusty_scene::Scene::new();
    let id = scene.add_entity("Fx".to_string());
    add_component(&mut scene, id, ComponentKind::Particles);
    (scene, id)
}

#[test]
fn the_cap_is_stored_as_given_however_it_is_set_and_holds_again_on_cpu() {
    let together = {
        let (mut scene, id) = emitter();
        set(
            &mut scene.world,
            id,
            json!({"simulation": "Gpu", "max_particles": 3_000_000}),
        );
        scene.world.particles(id).map(|p| p.config()).unwrap()
    };
    let (mut scene, id) = emitter();
    set(&mut scene.world, id, json!({"simulation": "Gpu"}));
    set(&mut scene.world, id, json!({"max_particles": 3_000_000}));
    let apart = scene.world.particles(id).map(|p| p.config()).unwrap();
    for p in [&together, &apart] {
        assert_eq!(p.max_particles, 3_000_000, "stored, not clamped");
        assert_eq!(
            p.particle_cap(),
            GPU_MAX_PARTICLES,
            "and held at the GPU's cap"
        );
    }
    set(&mut scene.world, id, json!({"simulation": "Cpu"}));
    assert_eq!(scene.world.particles(id).unwrap().particle_cap(), 3_000_000);
}

fn held_at(p: &ParticleEmitterComponent) -> usize {
    match p.simulation {
        ParticleSimulation::Cpu => p.runtime.particles.as_ptr() as usize,
        ParticleSimulation::Gpu => p.runtime.gpu.births().next().unwrap() as *const _ as usize,
    }
}

#[test]
fn a_set_of_one_field_neither_copies_nor_resets_the_live_particles() {
    for target in ParticleSimulation::ALL {
        let (mut scene, id) = emitter();
        set(&mut scene.world, id, json!({"simulation": target.name()}));
        let (at, epoch) = {
            let mut p = scene.world.particles_mut(id).unwrap();
            p.emit_at(EmitterFrame::IDENTITY, 40);
            assert!(draft(&p).runtime.particles.is_empty() && draft(&p).live_count() == 0);
            (held_at(&p), p.runtime.gpu.epoch())
        };
        set(&mut scene.world, id, json!({"rate": 5.0, "seed": 9}));
        let p = scene.world.particles(id).unwrap();
        assert_eq!((p.rate, p.seed), (5.0, 9), "{target:?}: the fields are set");
        assert_eq!(p.live_count(), 40, "{target:?}: nothing reset");
        assert_eq!(
            p.runtime.gpu.epoch(),
            epoch,
            "{target:?}: nor the renderer's buffer"
        );
        assert_eq!(
            held_at(&p),
            at,
            "{target:?}: the live particles moved back, uncopied"
        );
    }
}

#[test]
fn a_set_that_changes_target_or_space_forgets_as_its_op_does_and_a_failed_one_nothing() {
    let (mut scene, id) = emitter();
    scene
        .world
        .particles_mut(id)
        .unwrap()
        .emit_at(EmitterFrame::IDENTITY, 5);
    set(&mut scene.world, id, json!({"simulation_space": "Local"}));
    assert_eq!(
        scene.world.particles(id).unwrap().live_count(),
        5,
        "a Cpu move keeps them"
    );
    let mut bad = Map::new();
    bad.insert("simulation".into(), json!("Gpu"));
    bad.insert("rate".into(), json!("fast"));
    let kind = find("Particles").unwrap();
    assert!((kind.write)(&mut scene.world, id, &bad, Skip::Held).is_err());
    let p = scene.world.particles(id).unwrap();
    assert_eq!(
        (p.simulation, p.live_count()),
        (ParticleSimulation::Cpu, 5),
        "all or nothing"
    );
    drop(p);
    set(
        &mut scene.world,
        id,
        json!({"simulation": "Gpu", "simulation_space": "World"}),
    );
    let p = scene.world.particles(id).unwrap();
    assert_eq!(p.live_count(), 0, "a change of target forgets them");
    assert_eq!(p.simulation_space, rusty_components::SimulationSpace::World);
}

#[test]
fn a_change_of_space_forgets_a_gpu_emitters_particles_and_keeps_a_cpu_ones() {
    use rusty_components::SimulationSpace;
    let mut cpu = ParticleEmitterComponent::default();
    cpu.emit_at(EmitterFrame::IDENTITY, 5);
    set_simulation_space(&mut cpu, SimulationSpace::Local);
    assert_eq!(cpu.live_count(), 5);
    let mut gpu = ParticleEmitterComponent::default();
    set_simulation(&mut gpu, ParticleSimulation::Gpu);
    gpu.emit_at(EmitterFrame::IDENTITY, 5);
    let epoch = gpu.runtime.gpu.epoch();
    set_simulation_space(&mut gpu, SimulationSpace::Local);
    assert_eq!(gpu.live_count(), 0, "the old space's particles are gone");
    assert_ne!(
        gpu.runtime.gpu.epoch(),
        epoch,
        "and the renderer's buffer with them"
    );
    gpu.emit_at(EmitterFrame::IDENTITY, 2);
    set_simulation_space(&mut gpu, SimulationSpace::Local);
    assert_eq!(gpu.live_count(), 2, "no change, nothing forgotten");
}

#[test]
fn a_set_that_switches_to_gpu_warns_once_as_written_and_a_refused_one_not_at_all() {
    logs::init();
    let (mut scene, id) = emitter();
    let kind = find("Particles").unwrap();
    let mut bad = Map::new();
    bad.insert("simulation".into(), json!("Gpu"));
    bad.insert("collision".into(), json!("Bounce"));
    bad.insert("rate".into(), json!("fast"));
    assert!((kind.write)(&mut scene.world, id, &bad, Skip::Held).is_err());
    assert!(logs::take().is_empty(), "a refused write says nothing");
    set(
        &mut scene.world,
        id,
        json!({"simulation": "Gpu", "collision": "Bounce", "max_particles": 3_000_000}),
    );
    let said = logs::take();
    assert_eq!(
        said.len(),
        2,
        "the collision and the cap, once each: {said:?}"
    );
}
