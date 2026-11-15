//! crates/shell/src/editor/undrawn.rs — the editor's frame on a window that cannot
//! draw: the screen locked, the window occluded or minimised.
//!
//! The shell still runs that frame: it ticks the sim and drains the command channel,
//! so the attach bridge keeps answering. Only the paint is skipped, and the egui
//! pass with it, which is where the editor's per-frame work otherwise runs. This
//! does that work without painting, so work an agent asked for never waits on a
//! redraw: a `Lighting.GenerateAsync()` starts and steps to its end
//! (`EditorUi::undrawn_frame`), an edit reaches the navmesh, a file changed on disk
//! reaches the caches and the open scene follows it.

use super::EditorFrontend;
use rusty_app::GameWorld;

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
        // This window steps the bake whether or not it draws (`sync_gizmos` marks
        // it too, on a drawn frame).
        #[cfg(feature = "dev")]
        game.resources.application.borrow_mut().mark_bake_host();
        let playing = game.play_requested();
        let mut scene = game.world.scene.borrow_mut();
        let mut console = game.resources.console.borrow_mut();
        let mut nav = game.resources.nav.borrow_mut();
        let (scene, console, nav) = (&mut *scene, &mut *console, &mut *nav);
        let ctx = &self.egui_ctx;
        self.editor_ui
            .undrawn_frame(ctx, scene, console, nav, playing);
    }
}
