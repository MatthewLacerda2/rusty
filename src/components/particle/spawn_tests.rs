//! Where a particle is born (#1529): the shape's offset is placed around the
//! emitter, and a `Local` emitter's particle starts turned with it.

use glam::{Quat, Vec3};

use super::super::{EmitFrom, EmitShape, EmitterFrame, ParticleEmitterComponent, SimulationSpace};
use crate::core::curve::Range;

/// 90° about Y.
const TURN: Quat = Quat::from_xyzw(
    0.0,
    std::f32::consts::FRAC_1_SQRT_2,
    0.0,
    std::f32::consts::FRAC_1_SQRT_2,
);

fn emitter(space: SimulationSpace, shape: EmitShape) -> ParticleEmitterComponent {
    ParticleEmitterComponent {
        simulation_space: space,
        shape,
        emit_from: EmitFrom::Surface,
        speed: Range::constant(1.0),
        spread: 0.0,
        ..Default::default()
    }
}

/// The turn of one particle born under `frame`.
fn born_turn(space: SimulationSpace, frame: EmitterFrame) -> Quat {
    let mut e = emitter(space, EmitShape::Point);
    assert_eq!(e.emit_at(frame, 1), 1);
    e.runtime.particles[0].turn
}

#[test]
fn a_sphere_emitter_away_from_the_origin_launches_outward_from_where_each_is_born() {
    let at = Vec3::new(10.0, -3.0, 4.0);
    let mut e = emitter(SimulationSpace::World, EmitShape::Sphere { radius: 2.0 });
    e.emit_at(EmitterFrame::new(at, TURN, Vec3::ONE), 16);
    for p in &e.runtime.particles {
        let offset = p.position - at;
        assert!(
            (offset.length() - 2.0).abs() < 1e-4,
            "on the shell: {offset}"
        );
        assert!(
            offset.normalize().abs_diff_eq(p.velocity, 1e-4),
            "born at {offset} from the emitter, launched {}",
            p.velocity
        );
    }
}

#[test]
fn a_local_emitter_turned_before_a_burst_starts_its_particles_turned() {
    let turned = EmitterFrame::new(Vec3::new(1.0, 2.0, 3.0), TURN, Vec3::ONE);
    assert!(born_turn(SimulationSpace::Local, turned).abs_diff_eq(TURN, 1e-6));
}

#[test]
fn a_world_emitter_or_a_zero_scaled_one_starts_its_particles_unturned() {
    let turned = EmitterFrame::new(Vec3::ZERO, TURN, Vec3::ONE);
    assert_eq!(born_turn(SimulationSpace::World, turned), Quat::IDENTITY);
    // No usable inverse: the frame's turn could never be carried back out.
    let flat = EmitterFrame::new(Vec3::ZERO, TURN, Vec3::ZERO);
    assert_eq!(born_turn(SimulationSpace::Local, flat), Quat::IDENTITY);
}
