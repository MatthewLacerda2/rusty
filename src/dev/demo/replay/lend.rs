//! What a replay borrows from the host it plays in (#1598). [`prepare`](super::prepare)
//! writes the demo's start over the world's save data, video and graphics
//! settings, quality choice, play seed, clock and Game view size: harmless on a
//! headless run's throwaway world, but the editor's world is the operator's, and
//! its store is bound to `saved/storage.json`, flushed at every Stop.
//!
//! [`lend`] takes those out first and leaves the store pathless, so no Stop the
//! replay makes (a scrub back restarts it) writes the demo's data to disk;
//! [`Lent::give_back`] puts every one back as it was.

use crate::app::GameWorld;
use crate::core::settings::video::VideoSettings;
use crate::core::settings::GraphicsSettings;
use crate::core::storage::Storage;
use crate::time::Time;

/// The host's own state a replay overwrites, held while it plays.
pub struct Lent {
    /// The store as it was, its path with it.
    storage: Storage,
    video: VideoSettings,
    graphics: GraphicsSettings,
    quality: Option<String>,
    seed: u64,
    time: Time,
    game_view: Option<(u32, u32)>,
}

/// Take from `world` what a replay overwrites, leaving an empty pathless store and
/// no Game view size (a demo recorded headless lays out on its video resolution).
pub fn lend(world: &mut GameWorld) -> Lent {
    let storage = std::mem::take(&mut *world.resources.storage.borrow_mut());
    let mut screen = world.resources.screen.borrow_mut();
    let game_view = screen.game_view();
    screen.restore_game_view(None);
    drop(screen);
    let manager = world.script_manager();
    Lent {
        storage,
        video: *manager.video_cell().borrow(),
        graphics: *manager.graphics_cell().borrow(),
        quality: (world.resources.application.borrow())
            .quality_choice()
            .map(str::to_string),
        seed: manager.play_seed(),
        time: world.time().borrow().clone(),
        game_view,
    }
}

impl Lent {
    /// Put everything [`lend`] took back into `world`, after the replay left Play.
    pub fn give_back(self, world: &mut GameWorld) {
        *world.resources.storage.borrow_mut() = self.storage;
        let manager = world.script_manager();
        *manager.video_cell().borrow_mut() = self.video;
        *manager.graphics_cell().borrow_mut() = self.graphics;
        (world.resources.application.borrow_mut()).restore_quality(self.quality);
        world.resources.script_manager.set_play_seed(self.seed);
        *world.time().borrow_mut() = self.time;
        (world.resources.screen.borrow_mut()).restore_game_view(self.game_view);
    }
}
