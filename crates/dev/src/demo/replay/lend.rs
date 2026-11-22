use rusty_app::GameWorld;
use rusty_core::settings::video::VideoSettings;
use rusty_core::settings::GraphicsSettings;
use rusty_core::storage::Storage;
use rusty_time::Time;

/// What a replay writes over in its host: the editor's world is the operator's, and its
/// store flushes to `saved/storage.json` at every Stop.
pub struct Host {
    pub(super) storage: Storage,
    pub(super) video: VideoSettings,
    pub(super) graphics: GraphicsSettings,
    pub(super) quality: Option<String>,
    pub(super) seed: u64,
    pub(super) time: Time,
    pub(super) game_view: Option<(u32, u32)>,
    pub(super) display_scale: Option<f32>,
}

impl Host {
    /// Leaves the store empty and pathless, so no Stop a replay makes writes its data to disk.
    pub(super) fn take(world: &mut GameWorld) -> Self {
        let storage = std::mem::take(&mut *world.resources.storage.borrow_mut());
        let mut screen = world.resources.screen.borrow_mut();
        let game_view = screen.game_view();
        screen.restore_game_view(None);
        let display_scale = screen.reported_display_scale();
        screen.restore_display_scale(None);
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
            display_scale,
        }
    }

    /// Destructures every field, so one added to `Host` is put back by construction.
    pub(super) fn put(self, world: &mut GameWorld) {
        let Self {
            storage,
            video,
            graphics,
            quality,
            seed,
            time,
            game_view,
            display_scale,
        } = self;
        *world.resources.storage.borrow_mut() = storage;
        let manager = world.script_manager();
        *manager.video_cell().borrow_mut() = video;
        *manager.graphics_cell().borrow_mut() = graphics;
        (world.resources.application.borrow_mut()).restore_quality(quality);
        world.resources.script_manager.set_play_seed(seed);
        *world.time().borrow_mut() = time;
        let mut screen = world.resources.screen.borrow_mut();
        screen.restore_game_view(game_view);
        screen.restore_display_scale(display_scale);
    }
}

pub struct Lent(Host);

pub fn lend(world: &mut GameWorld) -> Lent {
    Lent(Host::take(world))
}

impl Lent {
    pub fn give_back(self, world: &mut GameWorld) {
        self.0.put(world);
    }
}
