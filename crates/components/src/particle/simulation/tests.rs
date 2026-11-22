use glam::Vec3;

use super::super::{EmitterFrame, ParticleEmitterComponent, ParticleSimulation};
use super::*;
use rusty_core::curve::Range;

fn particle(lifetime: f32) -> Particle {
    Particle::at(Vec3::ZERO, 1.0, [1.0; 4], lifetime)
}

#[derive(Default)]
struct Reference(Vec<(f32, f32)>);

impl Reference {
    fn step(&mut self, dt: f32) {
        self.0.retain_mut(|(age, lifetime)| {
            *age += dt;
            *age < *lifetime
        });
    }
}

#[test]
fn the_tally_ages_like_the_cpu_path_with_any_dt_and_lifetime() {
    let (mut sim, mut reference) = (GpuSim::default(), Reference::default());
    let lifetimes = [
        0.0,
        0.016,
        1.0 / 60.0,
        0.05,
        0.5,
        1.0,
        f32::NAN,
        f32::INFINITY,
        0.3,
    ];
    let dts = [1.0 / 60.0, 1.0 / 60.0, 0.0, 1.0 / 120.0, 0.05, 1.0 / 60.0];
    for tick in 0..400 {
        for (k, &l) in lifetimes.iter().enumerate() {
            if (tick + k) % 3 == 0 {
                sim.add(particle(l));
                reference.0.push((0.0, l));
            }
        }
        let dt = dts[tick % dts.len()];
        sim.step(dt);
        reference.step(dt);
        assert_eq!(sim.live(), reference.0.len(), "tick {tick}");
    }
    assert_eq!(sim.steps(), 400);
}

#[test]
fn live_slots_are_unique_and_reused_and_the_extent_resets_when_all_die() {
    let mut sim = GpuSim::default();
    for _ in 0..4 {
        sim.add(particle(1.0 / 60.0));
    }
    sim.add(particle(1.0));
    assert_eq!(sim.extent(), 5);
    sim.step(1.0 / 60.0);
    assert_eq!(sim.live(), 1);
    sim.add(particle(1.0));
    let live: Vec<u32> = sim.births().filter(|b| !b.dead).map(|b| b.slot).collect();
    assert_eq!(live.len(), 2, "the dead are gone from the births: {live:?}");
    assert_ne!(live[0], live[1]);
    assert_eq!(sim.extent(), 5, "a freed slot is reused before a fresh one");
    for _ in 0..61 {
        sim.step(1.0 / 60.0);
    }
    assert_eq!((sim.live(), sim.extent()), (0, 0));
    assert_eq!(sim.births().count(), 0);
}

#[test]
fn births_record_their_step_and_serial_and_dts_are_replayable() {
    let mut sim = GpuSim::default();
    sim.add(particle(5.0));
    sim.step(0.25);
    sim.step(0.5);
    sim.add(particle(5.0));
    let b: Vec<(u64, u64)> = sim.births().map(|b| (b.step, b.serial)).collect();
    assert_eq!(b, vec![(0, 0), (2, 1)]);
    assert_eq!(sim.next_serial(), 2);
    assert_eq!((sim.dt_of(1), sim.dt_of(2)), (Some(0.25), Some(0.5)));
    assert_eq!(
        sim.dt_of(0),
        Some(0.25),
        "older than the history: the oldest kept"
    );
    assert_eq!(GpuSim::default().dt_of(1), None);
}

#[test]
fn a_dead_birth_behind_a_live_one_waits_then_is_compacted() {
    let mut sim = GpuSim::default();
    sim.add(particle(f32::INFINITY));
    for _ in 0..3000 {
        sim.add(particle(0.0));
    }
    sim.step(1.0);
    assert_eq!(sim.live(), 1);
    assert!(
        sim.births().count() < 3000,
        "compacted past the live + 1024 bound"
    );
    assert!(sim.births().any(|b| !b.dead));
}

fn gpu_emitter(max: u32) -> ParticleEmitterComponent {
    ParticleEmitterComponent {
        simulation: ParticleSimulation::Gpu,
        max_particles: max,
        lifetime: Range::new(0.1, 0.9),
        ..Default::default()
    }
}

#[test]
fn a_gpu_emitter_spawns_through_the_seeded_path_into_births_under_its_cap() {
    let mut gpu = gpu_emitter(10);
    let mut cpu = ParticleEmitterComponent {
        simulation: ParticleSimulation::Cpu,
        ..gpu.clone()
    };
    assert_eq!(gpu.emit_at(EmitterFrame::IDENTITY, 25), 10, "the cap holds");
    assert_eq!(cpu.emit_at(EmitterFrame::IDENTITY, 25), 10);
    assert!(gpu.runtime.particles.is_empty());
    assert_eq!(gpu.live_count(), 10);
    let born: Vec<Vec3> = gpu
        .runtime
        .gpu
        .births()
        .map(|b| b.particle.position)
        .collect();
    let cpu_born: Vec<Vec3> = cpu.runtime.particles.iter().map(|p| p.position).collect();
    assert_eq!(born, cpu_born, "the same seeded stream");
    let epoch = gpu.runtime.gpu.epoch();
    gpu.clear_particles();
    assert_eq!(gpu.live_count(), 0);
    assert_ne!(gpu.runtime.gpu.epoch(), epoch, "Clear starts a new epoch");
}

#[test]
fn a_local_gpu_emitter_stores_its_births_relative_to_its_first_frame() {
    let mut e = ParticleEmitterComponent {
        simulation_space: super::super::SimulationSpace::Local,
        spread: 0.0,
        ..gpu_emitter(4)
    };
    let at = |x: f32| EmitterFrame::new(Vec3::X * x, glam::Quat::IDENTITY, Vec3::ONE);
    e.emit_at(at(5.0), 1);
    e.emit_at(at(7.0), 1);
    let stored: Vec<_> = e.runtime.gpu.births().map(|b| b.particle).collect();
    assert_eq!(
        stored[0].position,
        Vec3::X * 5.0,
        "at its first frame: as born"
    );
    assert!((stored[1].position - Vec3::X * 5.0).length() < 1e-5);
    assert!((stored[1].velocity - Vec3::Y * 2.0).length() < 1e-5);
    let drawn = e.gpu_space().transform_point3(stored[0].position);
    assert!((drawn - Vec3::X * 7.0).length() < 1e-5, "{drawn}");
}

#[test]
fn only_the_first_warning_is_owed() {
    let mut sim = GpuSim::default();
    assert!(sim.first_warning());
    assert!(!sim.first_warning());
    sim.clear();
    assert!(sim.first_warning());
}

#[test]
fn the_extent_going_back_to_0_is_counted_and_new_births_are_found_by_serial() {
    let mut sim = GpuSim::default();
    sim.step(0.1);
    assert_eq!(sim.emptied(), 0, "nothing ever lived: nothing to empty");
    for _ in 0..3 {
        sim.add(particle(0.05));
    }
    sim.add(particle(1.0));
    let from: Vec<u64> = sim.births_from(2).map(|b| b.serial).collect();
    assert_eq!(from, vec![2, 3]);
    sim.step(0.1);
    assert_eq!(sim.emptied(), 0, "one still lives");
    for _ in 0..10 {
        sim.step(0.1);
    }
    assert_eq!((sim.emptied(), sim.extent()), (1, 0));
    sim.step(0.1);
    assert_eq!(sim.emptied(), 1, "counted once per emptying");
}

#[test]
fn a_gpu_emitter_holds_no_more_than_the_fixed_cap() {
    let e = gpu_emitter(u32::MAX);
    assert_eq!(e.particle_cap(), GPU_MAX_PARTICLES);
    let cpu = ParticleEmitterComponent {
        simulation: ParticleSimulation::Cpu,
        ..e
    };
    assert_eq!(cpu.particle_cap(), u32::MAX);
}

#[test]
fn the_running_time_is_kept_at_each_recent_step_and_at_each_birth() {
    let mut sim = GpuSim::default();
    sim.step(0.25);
    sim.add(particle(1e9));
    for _ in 0..DT_HISTORY + 10 {
        sim.step(0.0);
    }
    sim.step(0.5);
    let born = sim.births().next().unwrap();
    assert_eq!(
        (born.step, born.time),
        (1, 0.25),
        "born after the first step"
    );
    assert_eq!(sim.time(), 0.75);
    let now = sim.steps();
    assert_eq!(sim.time_at(now), Some(0.75));
    assert_eq!(sim.time_at(now - 1), Some(0.25));
    assert_eq!(
        sim.time_at(now - DT_HISTORY as u64),
        Some(0.25),
        "the one before the kept dts"
    );
    assert_eq!(
        sim.time_at(now - DT_HISTORY as u64 - 1),
        None,
        "older than kept"
    );
    assert_eq!(sim.time_at(now + 1), None, "not taken yet");
    assert_eq!(GpuSim::default().time_at(0), Some(0.0));
}

fn local_twins_at_scale_0(before: &[f32], after: &[f32]) -> (Vec3, Vec3) {
    let local = |simulation| ParticleEmitterComponent {
        simulation,
        simulation_space: super::super::SimulationSpace::Local,
        shape: super::super::EmitShape::Sphere { radius: 1.0 },
        ..gpu_emitter(4)
    };
    let (mut gpu, mut cpu) = (
        local(ParticleSimulation::Gpu),
        local(ParticleSimulation::Cpu),
    );
    let scaled = |s: f32| EmitterFrame::new(Vec3::X * 10.0, glam::Quat::IDENTITY, Vec3::splat(s));
    for e in [&mut gpu, &mut cpu] {
        before.iter().for_each(|s| e.carry_to(scaled(*s)));
        e.emit_at(scaled(0.0), 1);
        after.iter().for_each(|s| e.carry_to(scaled(*s)));
    }
    let stored = gpu.runtime.gpu.births().next().unwrap().particle;
    let drawn = gpu.gpu_space().transform_point3(stored.position);
    (drawn, cpu.runtime.particles[0].position)
}

#[test]
fn a_local_gpu_birth_at_scale_0_draws_where_its_cpu_twin_holds_it() {
    let (drawn, carried) = local_twins_at_scale_0(&[2.0], &[1.0]);
    assert!((drawn - carried).length() < 1e-5, "{drawn} vs {carried}");
    assert!(
        (carried - Vec3::X * 10.0).length() < 1e-5,
        "born on the emitter: {carried}"
    );
    let (drawn, carried) = local_twins_at_scale_0(&[], &[1.0]);
    assert_eq!(carried, Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(drawn, carried, "not carried again under the first frame");
    let (drawn, carried) = local_twins_at_scale_0(&[], &[1.0, 2.0]);
    assert!((drawn - carried).length() < 1e-5, "{drawn} vs {carried}");
}
