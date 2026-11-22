//! The load reads finished only once all is made, so the new scene's first frame uploads
//! nothing.

use std::time::Instant;

use super::worker::Threaded;
use rusty_app::GameWorld;
use rusty_render::Renderer;

/// Warm the pending load for this frame's budget: at least one resource, then more
/// while the budget lasts and none waits on a worker.
pub fn warm_load(renderer: &mut Renderer, game: &mut GameWorld) {
    let application = game.resources.application.borrow();
    let budget = application.background_loading_priority().budget();
    drop(application);
    let Some(loader) = game.scene_loader_mut::<Threaded>() else {
        return;
    };
    let Some((data, warming)) = loader.to_warm() else {
        return;
    };
    let start = Instant::now();
    while renderer.warm_step(data, warming) && start.elapsed() < budget {}
}
