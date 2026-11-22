use super::EditorFrontend;
use rusty_app::GameWorld;
use rusty_editor::Paint;

impl EditorFrontend {
    /// Files changed outside the engine reach the caches, and the open scene its
    /// file, before a frame draws and on one that cannot.
    pub(super) fn follow_files(&mut self, game: &GameWorld) {
        if let Some(watch) = &mut self.watch {
            watch.apply(game.scene().borrow().is_playing());
        }
        self.follow_scene_file(game);
        self.remember_open_scene(game);
    }

    /// A frame the swapchain refused: everything `draw` does but the egui pass, the
    /// viewport's render and the paint.
    pub(super) fn step_undrawn(&mut self, game: &GameWorld) {
        self.follow_files(game);
        self.follow_scene_swap(game);
        let playing = game.play_requested();
        let mut scene = game.world.scene.borrow_mut();
        let mut console = game.resources.console.borrow_mut();
        let mut nav = game.resources.nav.borrow_mut();
        let (scene, console, nav) = (&mut *scene, &mut *console, &mut *nav);
        let ctx = &self.egui_ctx;
        let work = Paint::Undrawn;
        self.editor_ui
            .frame_work(ctx, scene, console, nav, playing, work);
    }
}
