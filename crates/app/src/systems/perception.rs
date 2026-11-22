use crate::resources::Resources;
use crate::world::World;

/// After physics and the post-animation pose, so the rays see where things stand now.
pub(crate) fn perceive(world: &mut World, res: &mut Resources) {
    let physics = res.physics.borrow();
    let mut scene = world.scene.borrow_mut();
    rusty_ai::perception::step(
        &mut scene,
        physics.as_ref(),
        &mut res.sight_rays,
        res.frame_dt,
    );
}

pub(crate) fn dispatch_perception_updates(world: &mut World, res: &mut Resources) {
    let updates = rusty_ai::perception::drain_updates(&mut world.scene.borrow_mut());
    res.script_manager.dispatch_perception_updates(&updates);
}

/// Every Play starts the round-robin from the first pair, so two runs trace the same
/// pairs on the same ticks.
pub(crate) fn reset_sight_rays(_world: &mut World, res: &mut Resources) {
    res.sight_rays.reset();
}
