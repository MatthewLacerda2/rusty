//! src/shell/frame/load/warm.rs — a `Scene.LoadAsync`'s GPU warm-up, a slice per frame
//! (#2123): the renderer makes what the prepared scene draws with
//! (`render::warm`) for up to the frame's budget, Unity's
//! `Application.backgroundLoadingPriority`, while the scene before it still draws.
//! The loader reads finished only once it is all made (`worker::Threaded`), so
//! the new scene's first frame uploads nothing. The budget is wall-clock and lives
//! here, on the platform side: it decides when an operation reads ready, an input a
//! demo records, never what the sim computes.

use std::time::Instant;

use super::worker::Threaded;
use crate::app::GameWorld;
use crate::render::Renderer;

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
