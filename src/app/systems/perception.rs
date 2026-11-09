//! The play schedule's perception systems (#1900): the sight step, the
//! `OnPerceptionUpdated` dispatch after it, and the Startup reset of the sight
//! round-robin. Where they sit in the tick is `app::play::register`'s call.

use crate::app::resources::Resources;
use crate::app::world::World;

/// The perception step (#1900): sight for every active `AIPerception`, on the
/// round-robin ray budget, then each memory ages by the tick's scaled dt. After
/// physics and the post-animation pose, so the rays see where things stand now.
pub(in crate::app) fn perceive(world: &mut World, res: &mut Resources) {
    let physics = res.physics.borrow();
    let mut scene = world.scene.borrow_mut();
    crate::ai::perception::step(
        &mut scene,
        physics.as_ref(),
        &mut res.sight_rays,
        res.frame_dt,
    );
}

/// `OnPerceptionUpdated(id, target, info)` (#1900) for every update the step, a
/// noise or a hit raised since the last dispatch: after `perceive`, before
/// `LateUpdate`, perceivers in id order.
pub(in crate::app) fn dispatch_perception_updates(world: &mut World, res: &mut Resources) {
    let updates = crate::ai::perception::drain_updates(&mut world.scene.borrow_mut());
    res.script_manager.dispatch_perception_updates(&updates);
}

/// Every Play session starts the sight round-robin from the first pair (#1900),
/// so two runs of the same scene trace the same pairs on the same ticks. (The
/// perceivers' memories were emptied on Play entry, `Scene::set_playing`.)
pub(in crate::app) fn reset_sight_rays(_world: &mut World, res: &mut Resources) {
    res.sight_rays.reset();
}
