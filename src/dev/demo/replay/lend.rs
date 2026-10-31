//! What a replay borrows from the host it plays in (#1598), as one list: [`Host`].
//! [`prepare`](super::prepare) writes the demo's start over the world's save data,
//! video and graphics settings, quality choice, play seed, clock and Game view
//! size: harmless on a headless run's throwaway world, but the editor's world is
//! the operator's, and its store is bound to `saved/storage.json`, flushed at
//! every Stop.
//!
//! That list is the host side of the scene state inventory (`scene::state`,
//! #1649), kept here because the scene can't name `Resources`. The replay's
//! install writes the demo's start *through* a [`Host`] ([`Host::take`], change,
//! [`Host::put`]), so it can only write what [`lend`] takes; and `put` names every
//! field, so a cell added to the list is put back by construction.
//!
//! [`lend`] takes it out first and leaves the store pathless, so no Stop the
//! replay makes (a scrub back restarts it) writes the demo's data to disk;
//! [`Lent::give_back`] puts every one back as it was.

use crate::app::GameWorld;
use crate::core::settings::video::VideoSettings;
use crate::core::settings::GraphicsSettings;
use crate::core::storage::Storage;
use crate::time::Time;

/// The host's own state a replay writes over.
pub struct Host {
    /// The store, its path with it.
    pub(super) storage: Storage,
    pub(super) video: VideoSettings,
    pub(super) graphics: GraphicsSettings,
    pub(super) quality: Option<String>,
    pub(super) seed: u64,
    pub(super) time: Time,
    pub(super) game_view: Option<(u32, u32)>,
}

impl Host {
    /// Take `world`'s host state, leaving an empty pathless store and no Game view
    /// size (a demo recorded headless lays out on its video resolution).
    pub(super) fn take(world: &mut GameWorld) -> Self {
        let storage = std::mem::take(&mut *world.resources.storage.borrow_mut());
        let mut screen = world.resources.screen.borrow_mut();
        let game_view = screen.game_view();
        screen.restore_game_view(None);
        drop(screen);
        let manager = world.script_manager();
        Self {
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

    /// Write every field into `world`.
    pub(super) fn put(self, world: &mut GameWorld) {
        let Self {
            storage,
            video,
            graphics,
            quality,
            seed,
            time,
            game_view,
        } = self;
        *world.resources.storage.borrow_mut() = storage;
        let manager = world.script_manager();
        *manager.video_cell().borrow_mut() = video;
        *manager.graphics_cell().borrow_mut() = graphics;
        (world.resources.application.borrow_mut()).restore_quality(quality);
        world.resources.script_manager.set_play_seed(seed);
        *world.time().borrow_mut() = time;
        (world.resources.screen.borrow_mut()).restore_game_view(game_view);
    }
}

/// The host's own state, held while a replay plays.
pub struct Lent(Host);

/// Take from `world` what a replay overwrites: its `Host` state.
pub fn lend(world: &mut GameWorld) -> Lent {
    Lent(Host::take(world))
}

impl Lent {
    /// Put everything [`lend`] took back into `world`, after the replay left Play.
    pub fn give_back(self, world: &mut GameWorld) {
        self.0.put(world);
    }
}
