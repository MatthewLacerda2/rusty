use glam::{Mat4, Quat, Vec3};

use crate::particle::{EmitterFrame, Particle, ParticleSimulation, SimulationSpace};
use crate::{Known, LineComponent, NavMeshAgentComponent, ParticleEmitterComponent, Sense};

const BY: Vec3 = Vec3::new(-4096.0, 0.0, 1024.0);

#[test]
fn an_agent_keeps_its_plan_in_the_shifted_space() {
    let mut a = NavMeshAgentComponent {
        target: Vec3::X * 5000.0,
        cached_path: vec![Vec3::X * 4000.0, Vec3::X * 5000.0],
        replan_from: Some(Vec3::X * 4500.0),
        ..Default::default()
    };
    a.planned_target = a.target;
    a.shift_origin(BY);
    assert_eq!(a.target, Vec3::X * 5000.0 + BY);
    assert_eq!(
        a.planned_target, a.target,
        "the plan still answers the target"
    );
    assert_eq!(a.cached_path[0], Vec3::X * 4000.0 + BY);
    assert_eq!(a.replan_from, Some(Vec3::X * 4500.0 + BY));
}

#[test]
fn a_line_moves_only_its_world_space_points() {
    let mut world = LineComponent::default();
    let mut local = LineComponent {
        use_world_space: false,
        ..Default::default()
    };
    world.shift_origin(BY);
    local.shift_origin(BY);
    assert_eq!(world.positions[0], BY);
    assert_eq!(local.positions[0], Vec3::ZERO);
}

#[test]
fn memory_moves_what_it_knows_and_what_it_will_report() {
    let mut m = crate::Memory::default();
    m.known.insert(7, Known::now(Sense::Sight, Vec3::ONE));
    m.shift_origin(BY);
    assert_eq!(m.known[&7].position, Vec3::ONE + BY);
}

fn emitter(space: SimulationSpace, simulation: ParticleSimulation) -> ParticleEmitterComponent {
    let mut e = ParticleEmitterComponent {
        simulation_space: space,
        simulation,
        ..Default::default()
    };
    let frame = EmitterFrame {
        matrix: Mat4::from_translation(Vec3::X * 5000.0),
        rotation: Quat::IDENTITY,
    };
    e.runtime.anchor = Some(frame);
    e.runtime.base = Some(frame);
    let p = Particle::at(Vec3::X * 5001.0, 1.0, [1.0; 4], 2.0);
    e.runtime.particles.push(p);
    e.runtime.gpu.add(p);
    e
}

#[test]
fn a_world_emitter_moves_its_particles_and_gpu_births() {
    let mut e = emitter(SimulationSpace::World, ParticleSimulation::Gpu);
    e.shift_origin(BY);
    assert_eq!(e.runtime.particles[0].position, Vec3::X * 5001.0 + BY);
    let born = e.runtime.gpu.births().next().unwrap().particle.position;
    assert_eq!(born, Vec3::X * 5001.0 + BY);
    assert_eq!(
        e.runtime.gpu.moved(),
        BY,
        "the renderer moves the uploaded ones"
    );
}

#[test]
fn a_local_emitter_carries_nothing_on_its_next_tick() {
    let mut e = emitter(SimulationSpace::Local, ParticleSimulation::Cpu);
    e.shift_origin(BY);
    let frame = EmitterFrame {
        matrix: Mat4::from_translation(Vec3::X * 5000.0 + BY),
        rotation: Quat::IDENTITY,
    };
    e.carry_to(frame);
    assert_eq!(e.runtime.particles[0].position, Vec3::X * 5001.0 + BY);
    assert_eq!(
        e.runtime.gpu.moved(),
        Vec3::ZERO,
        "local births ride the draw space"
    );
    let draw = e.gpu_space().transform_point3(Vec3::X * 5001.0);
    assert_eq!(
        draw,
        Vec3::X * 5001.0 + BY,
        "anchor over base carries the GPU births"
    );
}
