//! src/shell/editor/mod.rs — the editor frontend: the egui dashboard over the shell.
//!
//! The `rusty` binary. Each frame builds the egui UI (which lays out the viewport
//! panel), renders the scene into the offscreen viewport target sized to that panel,
//! applies click-to-select / gizmo drag, and paints the whole dashboard over the
//! swapchain (#183). Built only with the `editor` Cargo feature, which is what keeps
//! egui out of a shipped player.

// The headless editor capture (#731) sits here, above `editor`, so the arrow runs
// `editor → dev` only (#738).
#[cfg(feature = "dev")]
pub mod capture;
mod cursor_release;
mod game_focus;
mod icon;
mod paint;
mod start;
mod viewport;
mod watch;

#[cfg(feature = "dev")]
pub use start::capture_picker;
pub use start::{launch, launch_picker};

use winit::event::WindowEvent;
use winit::keyboard::KeyCode;

use super::frame::Host;
use super::{Frontend, Shell};
use crate::app::{GameWorld, PlayTransition};
use crate::core::input::CursorState;
use crate::editor::{EditorUi, ViewportInteraction, ViewportTab};
use crate::render::RenderView;
use crate::scene::SceneId;
use crate::shell::input::GameViewRect;

/// The editor's frontend state: egui, the editor UI, and its two offscreen views.
pub struct EditorFrontend {
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    editor_ui: EditorUi,
    /// Stable egui texture id for the offscreen scene target shown in the viewport
    /// panel (#183). Registered lazily on the first frame a target exists, then
    /// rebound to the resized view each frame so the `egui::Image` reference stays valid.
    viewport_texture_id: Option<egui::TextureId>,
    /// The viewport's per-view render state (its own target, depth, post-FX), one per
    /// tab, `[Scene, Game]`, as Unity keeps the two views apart: the Game view renders
    /// at the render scale and the Scene view natively (#1498), so a shared one would
    /// reallocate and reset its auto-exposure on every tab switch. Never shared with
    /// the preview view (#355). `None` until that tab has rendered once.
    viewport_views: [Option<RenderView>; 2],
    /// The Inspector Preview tab's texture id (#352) — same contract as the viewport's.
    preview_texture_id: Option<egui::TextureId>,
    /// The Inspector preview's own render state, independent of the viewport's (#355).
    preview_view: Option<RenderView>,
    /// Whether the Game view has input focus (#416; see `game_focus`).
    game_focused: bool,
    /// Whether Esc has freed the cursor over the game's lock (#576).
    cursor_release: cursor_release::CursorRelease,
    /// Where the Game view sat last frame, for mapping the pointer; `None` while the
    /// Scene tab is showing.
    game_view: Option<GameViewRect>,
    /// The scene identity the editor last drew, so a scene swapped in under it — a
    /// scripted `Scene.Load` (#432), Stop after one — drops the stale selection.
    scene_id: Option<SceneId>,
    /// The watch on the project's files (#1327); `None` when nothing could be watched.
    watch: Option<watch::Watch>,
    /// The open scene following its file on disk (#1340).
    follow: watch::follow::SceneFollow,
}

impl EditorFrontend {
    /// Set up egui on the shell's window and open the editor on the game's current
    /// scene file: the boot set it only if the scene loaded, so a startup scene that
    /// doesn't parse opens as Untitled (#1374).
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
        #[cfg_attr(not(feature = "dev"), expect(unused_mut, reason = "dev loads prefs"))]
        let mut editor_ui = EditorUi::new(game.script_manager().scene_path_cell());
        #[cfg(feature = "dev")]
        editor_ui.dev.load_prefs(&game.resources.storage.borrow());
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
        }
    }

    /// Run the editor UI for one frame and apply its post-FX quality selection.
    /// Returns the viewport panel's pointer interaction for picking/dragging (#183).
    fn draw_dashboard(&mut self, shell: &mut Shell, game: &mut GameWorld) -> ViewportInteraction {
        self.follow_scene_swap(game);
        // The transport asks through `set_playing`, like every other host (#1302).
        let mut playing = game.play_requested();
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
                shell.clock.fps(),
                shell.clock.frame_ms(),
                self.viewport_texture_id,
            )
        };
        game.set_playing(playing);
        crate::editor::build_settings::draw(
            &mut self.editor_ui,
            &self.egui_ctx,
            &mut game.resources.application.borrow_mut(),
            &mut game.script_manager().graphics_cell().borrow_mut().options,
            &mut game.resources.console.borrow_mut(),
        );
        if self.editor_ui.is_dirty {
            shell.renderer.shadow_renderer.invalidate_static_cache();
            self.editor_ui.is_dirty = false;
        }
        self.sync_quality(shell, game);
        self.sync_speaker_mode(game);
        #[cfg(feature = "dev")]
        self.editor_ui
            .dev
            .store_prefs(&mut game.resources.storage.borrow_mut());
        interaction
    }

    /// The Config menu and `Graphics.SetQuality` (script) select through one
    /// `Application`: select a preset picked in the menu, mirror the presets and the
    /// running one back into it, then hand the shared cell to the renderer
    /// (`settings::apply_graphics`), the player's graphics options beside the
    /// preset's values (#1438, #1522).
    fn sync_quality(&mut self, shell: &mut Shell, game: &GameWorld) {
        let mut app = game.resources.application.borrow_mut();
        if let Some(name) = self.editor_ui.quality_request.take() {
            if let Err(err) = app.select_quality(&name) {
                log::warn!("[Quality] {err}");
            }
            game.script_manager().graphics_cell().borrow_mut().quality = app.quality_values();
        }
        let names = app.quality_presets().names();
        self.editor_ui.quality_presets = names.into_iter().map(str::to_string).collect();
        self.editor_ui.quality_preset = app.quality().map(str::to_string);
        drop(app);
        super::settings::apply_graphics(shell, game);
    }

    /// Apply a speaker mode picked in Config, then mirror the maestro's mode (which
    /// `Audio.SetSpeakerMode` may also have changed) back into the menu.
    fn sync_speaker_mode(&mut self, game: &GameWorld) {
        let mut audio = game.resources.audio.borrow_mut();
        if let Some(mode) = self.editor_ui.speaker_mode_request.take() {
            audio.set_speaker_mode(mode);
        }
        self.editor_ui.speaker_mode = audio.speaker_mode();
    }

    /// A different scene now sits in the World (its identity changed without the
    /// editor opening it): as File ▸ Open does, drop the selection, which names an
    /// entity of the old scene.
    fn follow_scene_swap(&mut self, game: &GameWorld) {
        let id = game.scene().borrow().id();
        if self.scene_id.is_some_and(|last| last != id) {
            self.editor_ui.selected_entity_id = None;
        }
        self.scene_id = Some(id);
    }
}

impl Frontend for EditorFrontend {
    const HOST: Host = Host::Editor;

    /// Every event reaches egui, so the header's Play/Stop buttons always work.
    fn on_window_event(&mut self, shell: &Shell, event: &WindowEvent) {
        let _ = self.egui_winit.on_window_event(&shell.window, event);
    }

    /// Raw mouse motion feeds egui too, so Scene-view look keeps turning while the
    /// cursor is locked (#745).
    fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.egui_winit.on_mouse_motion(delta);
    }

    /// The game hears input only while playing with the Game view focused.
    fn game_has_input(&self, game: &GameWorld) -> bool {
        game.play_requested() && self.game_focused
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

    /// The game's request, unless Esc freed the cursor (#576). RMB mouse look in the
    /// Scene view hides and locks it, as Unity does (#745).
    fn cursor(&self, game: &GameWorld) -> CursorState {
        if self.editor_ui.scene_view.looking {
            return CursorState::PLAY;
        }
        let requested = game.input().borrow().cursor();
        self.cursor_release
            .effective(requested, self.game_has_input(game))
    }

    /// Esc in Play frees the cursor and still reaches the game (Unity's behaviour);
    /// Ctrl/Cmd+P or the toolbar stops Play.
    fn on_key(&mut self, game: &mut GameWorld, key: KeyCode, pressed: bool) {
        let has_input = self.game_has_input(game);
        self.cursor_release.on_key(key, pressed, has_input);
    }

    fn draw(
        &mut self,
        shell: &mut Shell,
        game: &mut GameWorld,
        _frame: &wgpu::Texture,
        target: &wgpu::TextureView,
    ) {
        // Files changed outside the engine reach the caches before this frame draws.
        if let Some(watch) = &mut self.watch {
            watch.apply(game.scene().borrow().is_playing());
        }
        self.follow_scene_file(game);
        let raw_input = self.egui_winit.take_egui_input(&shell.window);
        self.egui_ctx.begin_pass(raw_input);

        self.editor_ui.preview_texture_id = self.preview_texture_id;
        sync_ui_textures(&mut self.editor_ui, game);
        let was_dirty = self.editor_ui.is_dirty;
        let interaction = self.draw_dashboard(shell, game);
        if self.editor_ui.is_dirty && !was_dirty && game.scene().borrow().is_playing() {
            // An edit by hand is no API call a demo can replay (#1581).
            let what = "an editor edit (the Inspector, the hierarchy, a gizmo)".to_string();
            let journal = game.script_manager().journal();
            journal
                .borrow_mut()
                .push(crate::scripting::HostCall::Taint(what));
        }
        paint::drain_repl(&mut self.editor_ui, game);

        // Render the scene into the viewport target now (after the panel rect is
        // known, before egui paints) so the `egui::Image` samples this frame's render.
        let ppp = shell.window.scale_factor() as f32;
        self.update_game_focus(game, &interaction);
        self.game_view = (interaction.tab == ViewportTab::Game)
            .then(|| viewport::target_size(interaction.size, ppp))
            .flatten()
            .map(|render| game_focus::game_view_rect(&interaction, ppp, render));
        self.render_viewport_scene(shell, game, &interaction, ppp);
        self.handle_viewport_interaction(game, &interaction, ppp);
        // Only when the Inspector's Preview tab requested one this frame (#352).
        self.render_preview_scene(shell, ppp);

        self.paint(shell, target);
    }
}

/// Hand the sim's texture-size table to the editor, so the overlay and the
/// inspector lay the UI out with the very table the game does (#1221).
pub(super) fn sync_ui_textures(ui: &mut EditorUi, game: &GameWorld) {
    ui.ui_textures = std::rc::Rc::clone(game.resources.screen.borrow().textures());
}
