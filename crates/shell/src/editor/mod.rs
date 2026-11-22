#[cfg(feature = "dev")]
pub mod capture;
mod cursor_release;
#[cfg(feature = "dev")]
pub mod demo;
mod game_focus;
mod icon;
mod last_scene;
mod navmesh;
mod paint;
mod start;
mod sync;
mod title;
mod undo;
mod undrawn;
mod viewport;
mod watch;

#[cfg(feature = "dev")]
pub use start::capture_picker;
pub use start::{launch, launch_picker};

use winit::event::WindowEvent;
use winit::keyboard::KeyCode;

#[cfg(feature = "dev")]
use super::frame::Step;
use super::{frame::Host, pace::Pace, Frontend, Shell};
use crate::input::GameViewRect;
use rusty_app::{GameWorld, PlayTransition};
use rusty_core::input::CursorState;
use rusty_editor::{EditorUi, ViewportInteraction};
use rusty_render::RenderView;
use rusty_scene::SceneId;

pub struct EditorFrontend {
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    editor_ui: EditorUi,
    viewport_texture_id: Option<egui::TextureId>,
    /// `[Scene, Game]`: they render at different scales, so a shared view would resize,
    /// cutting auto-exposure, on each tab switch.
    viewport_views: [Option<RenderView>; 2],
    preview_texture_id: Option<egui::TextureId>,
    preview_view: Option<RenderView>,
    game_focused: bool,
    cursor_release: cursor_release::CursorRelease,
    game_view: Option<GameViewRect>,
    scene_id: Option<SceneId>,
    watch: Option<watch::Watch>,
    follow: watch::follow::SceneFollow,
    last_scene: Option<String>,
    repaint_at: Option<std::time::Instant>,
    title: title::Title,
}

impl EditorFrontend {
    pub fn new(shell: &Shell, game: &GameWorld) -> Self {
        let egui_ctx = egui::Context::default();
        let egui_winit = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &shell.window,
            Some(shell.window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            &shell.renderer.device,
            shell.renderer.config.format,
            paint::EGUI_RENDERER,
        );
        let editor_ui = EditorUi::new(game.script_manager().scene_path_cell());
        sync::mark_host(&mut game.resources.application.borrow_mut());
        Self {
            egui_ctx,
            egui_winit,
            egui_renderer,
            editor_ui,
            viewport_texture_id: None,
            viewport_views: [None, None],
            preview_texture_id: None,
            preview_view: None,
            game_focused: false,
            cursor_release: Default::default(),
            game_view: None,
            scene_id: None,
            watch: watch::Watch::editor(),
            follow: watch::follow::SceneFollow::new(),
            last_scene: None,
            repaint_at: None,
            title: title::Title::new(),
        }
    }

    /// Run the editor UI for one frame and apply its post-FX quality selection.
    fn draw_dashboard(&mut self, shell: &mut Shell, game: &mut GameWorld) -> ViewportInteraction {
        self.follow_scene_swap(game);
        let mut playing = game.play_requested();
        let clock = (shell.clock.fps(), shell.clock.frame_ms());
        let timestamps = shell.renderer.times_gpu();
        sync::sync_stats(&mut self.editor_ui, game, clock.into(), timestamps);
        let interaction = {
            let mut s = game.world.scene.borrow_mut();
            let mut c = game.resources.console.borrow_mut();
            let mut n = game.resources.nav.borrow_mut();
            self.editor_ui.draw(
                &self.egui_ctx,
                &mut s,
                &mut c,
                &mut n,
                &mut playing,
                self.viewport_texture_id,
            )
        };
        game.set_playing(playing);
        rusty_editor::build_settings::draw(
            &mut self.editor_ui,
            &self.egui_ctx,
            &mut game.resources.application.borrow_mut(),
            &mut game.script_manager().graphics_cell().borrow_mut(),
            &mut game.resources.console.borrow_mut(),
        );
        if paint::take_edits(&mut self.editor_ui, game) {
            shell.renderer.shadow_renderer.invalidate_static_cache();
        }
        if self.demo_size().is_none() {
            // Before `sync_shared` resolves the render scale on it; a demo keeps its recorded one.
            let scale =
                (self.editor_ui.game_view).display_scale(shell.window.scale_factor() as f32);
            game.resources.screen.borrow_mut().set_display_scale(scale);
        }
        self.sync_shared(shell, game);
        self.title.sync(&shell.window, &mut self.editor_ui, game);
        interaction
    }
}

impl Frontend for EditorFrontend {
    const HOST: Host = Host::Editor;

    /// Every event reaches egui, so the header's Play/Stop buttons always work.
    fn on_window_event(&mut self, shell: &Shell, event: &WindowEvent) {
        let _ = self.egui_winit.on_window_event(&shell.window, event);
    }

    /// Raw mouse motion feeds egui too, so Scene-view look keeps turning while the
    /// cursor is locked.
    fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.egui_winit.on_mouse_motion(delta);
    }

    fn game_has_input(&self, game: &GameWorld) -> bool {
        #[cfg(feature = "dev")]
        if self.editor_ui.demo_open() {
            return false;
        }
        game.play_requested() && self.game_focused
    }

    #[cfg(feature = "dev")]
    fn advance(&mut self, game: &mut GameWorld, mut step: Step<'_>, dt: f32) -> PlayTransition {
        let demo = demo::advance(&mut self.editor_ui, game, dt).inspect(|_| step.clear());
        demo.unwrap_or_else(|| super::frame::advance_sim(game, step, dt))
    }

    #[cfg(feature = "dev")]
    fn before_exit(&mut self, game: &mut GameWorld) {
        demo::close(&mut self.editor_ui, game);
    }

    fn pace(&self, game: &GameWorld) -> Pace {
        #[cfg(feature = "dev")]
        if self.editor_ui.demo_open() {
            return Pace::Continuous;
        }
        let playing = game.play_requested() || game.scene().borrow().is_playing();
        if playing || game.resources.audio.borrow().is_sounding() {
            return Pace::Continuous;
        }
        Pace::OnDemand {
            next: self.repaint_at,
        }
    }

    fn game_view(&self, shell: &Shell) -> GameViewRect {
        self.game_view.unwrap_or_else(|| {
            let config = &shell.renderer.config;
            GameViewRect::full_window(config.width, config.height)
        })
    }

    fn on_play_transition(&mut self, _game: &mut GameWorld, transition: PlayTransition) {
        self.focus_on_transition(transition);
    }

    fn cursor(&self, game: &GameWorld) -> CursorState {
        if self.editor_ui.scene_view.looking {
            return CursorState::PLAY;
        }
        let requested = game.input().borrow().cursor();
        self.cursor_release
            .effective(requested, self.game_has_input(game))
    }

    fn on_key(&mut self, game: &mut GameWorld, key: KeyCode, pressed: bool) {
        let has_input = self.game_has_input(game);
        self.cursor_release.on_key(key, pressed, has_input);
    }

    /// The locked screen's frame: the editor's work without the paint (`undrawn`).
    fn undrawn(&mut self, _shell: &mut Shell, game: &mut GameWorld) {
        self.step_undrawn(game);
    }

    fn draw(
        &mut self,
        shell: &mut Shell,
        game: &mut GameWorld,
        _frame: &wgpu::Texture,
        target: &wgpu::TextureView,
    ) {
        self.follow_files(game);
        let raw_input = self.egui_winit.take_egui_input(&shell.window);
        self.egui_ctx.begin_pass(raw_input);

        self.editor_ui.preview_texture_id = self.preview_texture_id;
        sync_ui_textures(&mut self.editor_ui, game);
        let interaction = self.recorded_dashboard(shell, game);
        paint::drain_repl(&mut self.editor_ui, game);

        let ppp = shell.window.scale_factor() as f32;
        self.update_game_focus(game, &interaction);
        self.game_view = game_focus::game_view(&interaction, ppp);
        // Now: the layout gave the panel rect, and egui samples this render when it paints.
        self.render_viewport_scene(shell, game, &interaction, ppp);
        self.recorded_viewport_interaction(game, &interaction, ppp);
        self.render_preview(shell, ppp);

        self.paint(shell, target);
    }
}

pub(super) fn sync_ui_textures(ui: &mut EditorUi, game: &GameWorld) {
    ui.ui_textures = std::rc::Rc::clone(game.resources.screen.borrow().textures());
}
