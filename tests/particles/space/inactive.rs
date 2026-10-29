//! An inactive `Local` emitter (#1529) is not carried: its particles hold still
//! while the entity is off, as Unity's deactivated Particle System does.

use glam::Vec3;
use rusty::components::particle::SimulationSpace;

use super::{burst, state};

#[test]
fn an_inactive_local_emitter_moved_leaves_its_particles_where_they_were() {
    let (mut world, id) = burst(SimulationSpace::Local, 0.0);
    let before = state(&world, id);
    {
        let mut scene = world.scene().borrow_mut();
        scene.world.set_active(id, false);
        scene.world.transform_mut(id).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
    }
    world.tick(1.0 / 60.0);
    assert_eq!(state(&world, id), before);
}
