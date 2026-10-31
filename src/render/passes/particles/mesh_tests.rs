//! The matrix a mesh particle is drawn with (#440, #1529): at the particle, its
//! size, turned by the `Local` emitter's turn on top of its own tumble.

use glam::{Quat, Vec3};

use super::mesh_particle_matrix;
use crate::components::particle::{EmitterFrame, ParticleRenderMode, SimulationSpace};
use crate::components::ParticleEmitterComponent;
use crate::core::curve::{Curve, Range};

#[test]
fn a_local_mesh_particle_is_drawn_at_its_size_turned_with_the_emitter() {
    let mut e = ParticleEmitterComponent {
        simulation_space: SimulationSpace::Local,
        size: Range::constant(0.5),
        rotation: Range::new(0.0, 180.0),
        ..Default::default()
    };
    e.render.mode = ParticleRenderMode::Mesh;
    e.emit_at(EmitterFrame::IDENTITY, 4);
    let turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    e.carry_to(EmitterFrame::new(Vec3::new(5.0, 0.0, 0.0), turn, Vec3::ONE));
    for p in &e.runtime.particles {
        let tumble = Quat::from_axis_angle(p.axis.normalize(), p.rotation);
        let m = mesh_particle_matrix(p.position, e.size_of(p), p.orientation());
        assert!(m.transform_point3(Vec3::ZERO).abs_diff_eq(p.position, 1e-5));
        let x = m.transform_vector3(Vec3::X);
        let want = turn * tumble * Vec3::X * 0.5;
        assert!(x.abs_diff_eq(want, 1e-5), "drawn X {x}, want {want}");
    }
}

/// A size curve that dips below zero draws the particle at zero, never mirrored
/// (#1615): a mirrored mesh would show its inside with back faces culled.
#[test]
fn a_size_curve_below_zero_draws_a_mesh_particle_at_zero_not_mirrored() {
    let mut e = ParticleEmitterComponent {
        size: Range::constant(1.0),
        size_over_life: Curve::constant(-2.0),
        ..Default::default()
    };
    e.render.mode = ParticleRenderMode::Mesh;
    e.emit_at(EmitterFrame::IDENTITY, 1);
    let p = &e.runtime.particles[0];
    assert_eq!(e.size_of(p), 0.0);
    let m = mesh_particle_matrix(p.position, e.size_of(p), p.orientation());
    assert!(m.determinant() >= 0.0, "a mirrored mesh particle: {m}");
}
