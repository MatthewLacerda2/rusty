//! src/app/decals.rs — the decal ageing tick (#639).
//!
//! Every decal ages by the fixed tick's scaled `dt` (so a paused game freezes its
//! fades, and a headless replay ages them identically), and the ones whose lifetime
//! ran out or whose owner is gone are dropped. It runs after `apply_destroys`, so a
//! door destroyed this tick takes its bullet holes with it the same tick.

use super::resources::Resources;
use super::world::World;

/// Age the scene's decals by `res.frame_dt` and drop the finished ones.
pub(super) fn tick_decals(world: &mut World, res: &mut Resources) {
    let mut scene = world.scene.borrow_mut();
    let scene = &mut *scene;
    let entities = &scene.world;
    scene.decals.tick(res.frame_dt, |id| entities.contains(id));
}
