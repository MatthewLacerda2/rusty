use crate::resources::Resources;
use crate::world::World;

/// After `apply_destroys`, so a destroyed owner's decals go the same tick.
pub(crate) fn tick_decals(world: &mut World, res: &mut Resources) {
    let mut scene = world.scene.borrow_mut();
    let scene = &mut *scene;
    let entities = &scene.world;
    scene.decals.tick(res.frame_dt, |id| entities.contains(id));
}
