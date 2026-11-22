//! GPU-simulated emitters with no adapter: the sim's tally matches a `Cpu`
//! twin tick by tick through the real play loop, the `Particles` pair and the
//! calls a `Gpu` emitter answers differently, and the field surviving save/load.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;
use rusty::components::particle::{
    EmitMode, EmitShape, ParticleEmitterComponent, ParticleSimulation,
};
use rusty::core::curve::Range;
use rusty::scene::Scene;
use rusty::script::api::bind;

fn emitter(simulation: ParticleSimulation, mode: EmitMode) -> ParticleEmitterComponent {
    ParticleEmitterComponent {
        simulation,
        emit_mode: mode,
        looping: true,
        rate: 90.0,
        burst_count: 40,
        max_particles: 60,
        shape: EmitShape::Sphere { radius: 1.0 },
        lifetime: Range::new(0.2, 1.1),
        speed: Range::new(0.5, 2.0),
        gravity: Vec3::new(0.0, -3.0, 0.0),
        seed: 9,
        ..Default::default()
    }
}

fn scene_of(e: ParticleEmitterComponent) -> (Rc<RefCell<Scene>>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Smoke".to_string());
    scene.world.set_particles(id, Some(e));
    (Rc::new(RefCell::new(scene)), id)
}

fn count(scene: &Rc<RefCell<Scene>>, id: u32) -> usize {
    scene.borrow().world.particles(id).unwrap().live_count()
}

#[test]
fn the_gpu_tally_matches_a_cpu_twin_tick_by_tick_through_the_play_loop() {
    for mode in [EmitMode::Continuous, EmitMode::Burst] {
        let (gpu, g) = scene_of(emitter(ParticleSimulation::Gpu, mode));
        let (cpu, c) = scene_of(emitter(ParticleSimulation::Cpu, mode));
        let mut gw = crate::play::playing(gpu.clone(), 20.0);
        let mut cw = crate::play::playing(cpu.clone(), 20.0);
        let mut peak = 0;
        for tick in 0..240 {
            crate::play::run(&mut gw, 1);
            crate::play::run(&mut cw, 1);
            assert_eq!(count(&gpu, g), count(&cpu, c), "{mode:?} tick {tick}");
            peak = peak.max(count(&gpu, g));
        }
        assert!(peak > 30, "{mode:?} emitted: {peak}");
        let e = gpu.borrow().world.particles(g).unwrap().clone();
        assert!(
            e.runtime.particles.is_empty(),
            "the sim keeps no live particle"
        );
    }
}

/// Run `code` against `scene` with the `Particles` namespace registered.
fn lua<R: mlua::FromLuaMulti>(scene: &Rc<RefCell<Scene>>, code: &str) -> mlua::Result<R> {
    let lua = Lua::new();
    crate::lua::attempt(|| {
        rusty::script::api::particle::register(&lua, bind(&lua, scene)).unwrap();
        lua.load(code).eval::<R>()
    })
}

#[test]
fn the_lua_pair_round_trips_and_a_gpu_emitter_answers_emit_count_and_clear_exactly() {
    let (scene, id) = scene_of(emitter(ParticleSimulation::Cpu, EmitMode::Burst));
    let get = format!("return Particles.GetSimulation({id})");
    assert_eq!(lua::<String>(&scene, &get).unwrap(), "Cpu");
    lua::<()>(&scene, &format!("Particles.SetSimulation({id}, 'gpu')")).unwrap();
    assert_eq!(lua::<String>(&scene, &get).unwrap(), "Gpu");
    let bad = lua::<()>(&scene, &format!("Particles.SetSimulation({id}, 'Quantum')"));
    assert!(bad.unwrap_err().to_string().contains("want one of"));
    let script = format!(
        "local a = Particles.Emit({id}, 25)
         local b = Particles.Emit({id}, 100)
         return a, b, Particles.GetCount({id}), #Particles.GetParticles({id}),
                Particles.SetParticles({id}, {{}})"
    );
    let (a, b, n, read, written) = lua::<(u32, u32, u32, u32, u32)>(&scene, &script).unwrap();
    assert_eq!((a, b, n), (25, 35, 60), "the cap of 60 holds, exactly");
    assert_eq!(read, 0, "a Gpu emitter's particles are the renderer's");
    assert_eq!(
        (written, count(&scene, id)),
        (60, 60),
        "SetParticles changes nothing"
    );
    let arrays = format!(
        "local a = Particles.GetArrays({id}, {{ 'x', 'vy' }})
         return #a.x, Particles.SetArrays({id}, a), Particles.ForEach({id}, function() end)"
    );
    let (read, set, each) = lua::<(u32, u32, u32)>(&scene, &arrays).unwrap();
    assert_eq!(
        (read, set, each),
        (0, 60, 60),
        "an empty read; writes change nothing"
    );
    lua::<()>(&scene, &format!("Particles.Clear({id})")).unwrap();
    assert_eq!(count(&scene, id), 0);
}

#[test]
fn the_field_survives_save_and_load() {
    let (scene, id) = scene_of(emitter(ParticleSimulation::Gpu, EmitMode::Burst));
    let path = crate::temp::dir().join("rusty_particle_gpu.scene");
    let path = path.to_string_lossy().into_owned();
    rusty::edit::save_to_file(&scene.borrow(), &path).unwrap();
    let mut loaded = Scene::new();
    rusty::edit::load_from_file(&mut loaded, &path).unwrap();
    let e = loaded.world.entity_document(id).unwrap();
    assert_eq!(e.particles.unwrap().simulation, ParticleSimulation::Gpu);
}
