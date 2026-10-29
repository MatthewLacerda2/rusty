//! Component snapshots an agent reads back after authoring them.

use super::particle_value;
use crate::components::particle::{EmitShape, SimulationSpace};
use crate::components::ParticleEmitterComponent;
use glam::Vec3;

#[test]
fn a_particle_emitter_reads_back_what_was_authored() {
    let p = ParticleEmitterComponent {
        rate: 7.0,
        max_particles: 12,
        simulation_space: SimulationSpace::Local,
        direction: Vec3::X,
        shape: EmitShape::Sphere { radius: 2.0 },
        ..Default::default()
    };
    let v = particle_value(&p);
    assert_eq!(v["rate"].as_f64(), Some(7.0));
    assert_eq!(v["max_particles"].as_u64(), Some(12));
    assert_eq!(v["simulation_space"].as_str(), Some("Local"));
    assert_eq!(v["direction"][0].as_f64(), Some(1.0));
    assert_eq!(v["shape"].as_str(), Some("Sphere"));
    assert_eq!(v["active"].as_bool(), Some(p.active));
}
