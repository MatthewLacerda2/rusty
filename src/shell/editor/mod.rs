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
mod paint;
mod viewport;

use winit::event::WindowEvent;
use winit::keyboard::KeyCode;

use super::frame::Host;
use super::{boot, Frontend, Launch, Shell};
use crate::app::{GameWorld, PlayTransition};
use crate::core::application::BUILD_SETTINGS_PATH;
use crate::core::input::CursorState;
use crate::core::video::VideoSettings;
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
    /// The viewport's per-view render state (its own target, depth, post-FX); never
    /// shared with the preview view (#355). `None` until the viewport has rendered once.
    viewport_view: Option<RenderView>,
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
}

impl EditorFrontend {
    /// Set up egui on the shell's window and open the editor on `scene_path`.
    pub fn new(shell: &Shell, game: &GameWorld, scene_path: String) -> Self {
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
        let mut editor_ui = EditorUi::new();
        editor_ui.current_scene_path = Some(scene_path);
        editor_ui.quality_preset = *game.script_manager().quality_cell().borrow();
        Self {
            egui_ctx,
            egui_winit,
            egui_renderer,
            editor_ui,
            viewport_texture_id: None,
            viewport_view: None,
            preview_texture_id: None,
            preview_view: None,
            game_focused: false,
            cursor_release: Default::default(),
            game_view: None,
            scene_id: None,
        }
    }

    /// Run the editor UI for one frame and apply its post-FX quality selection.
    /// Returns the viewport panel's pointer interaction for picking/dragging (#183).
    fn draw_dashboard(&mut self, shell: &mut Shell, game: &mut GameWorld) -> ViewportInteraction {
        self.follow_scene_swap(game);
        let interaction = {
            let mut s = game.world.scene.borrow_mut();
            let mut c = game.resources.console.borrow_mut();
            let mut n = game.resources.nav.borrow_mut();
            self.editor_ui.draw(
                &self.egui_ctx,
                &mut s,
                &mut c,
                &mut n,
                &mut game.resources.is_playing,
                shell.clock.fps(),
                shell.clock.frame_ms(),
                self.viewport_texture_id,
            )
        };
        crate::editor::build_settings::draw(
            &mut self.editor_ui,
            &self.egui_ctx,
            &mut game.resources.application.borrow_mut(),
            &mut game.resources.console.borrow_mut(),
        );
        if self.editor_ui.is_dirty {
            shell.renderer.shadow_renderer.invalidate_static_cache();
            self.editor_ui.is_dirty = false;
        }
        self.sync_quality(shell, game);
        self.sync_speaker_mode(game);
        self.sync_scene_path(game);
        interaction
    }

    /// The editor dropdown and `Graphics.SetQuality` (script) feed one shared cell:
    /// reflect a script-driven change back into the dropdown, push the editor's choice
    /// into the cell, then hand the result to the renderer (`set_quality` guards the
    /// bloom realloc).
    fn sync_quality(&mut self, shell: &mut Shell, game: &GameWorld) {
        let cell = game.script_manager().quality_cell();
        let scripted = *cell.borrow();
        if scripted != self.editor_ui.quality_preset
            && self.editor_ui.quality_preset == shell.renderer.quality
        {
            self.editor_ui.quality_preset = scripted;
        }
        *cell.borrow_mut() = self.editor_ui.quality_preset;
        shell.renderer.set_quality(self.editor_ui.quality_preset);
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

    /// Keep the script-side `Scene.Save()` write-back target in lockstep with the
    /// editor's current scene file. A scripted Save-with-path (Save As) or
    /// `Scene.Load` (#432) updates the cell; reflect that back into the editor so
    /// both agree on the file.
    fn sync_scene_path(&mut self, game: &GameWorld) {
        let path_cell = game.script_manager().scene_path_cell();
        let scripted_path = path_cell.borrow().clone();
        if scripted_path.is_some() && scripted_path != self.editor_ui.current_scene_path {
            self.editor_ui.current_scene_path = scripted_path;
        }
        *path_cell.borrow_mut() = self.editor_ui.current_scene_path.clone();
    }
}

impl Frontend for EditorFrontend {
    const HOST: Host = Host::Editor;

    /// Every event reaches egui, so the header's Play/Stop buttons always work.
    fn on_window_event(&mut self, shell: &Shell, event: &WindowEvent) {
        let _ = self.egui_winit.on_window_event(&shell.window, event);
    }

    /// The game hears input only while playing with the Game view focused.
    fn game_has_input(&self, game: &GameWorld) -> bool {
        game.is_playing() && self.game_focused
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

    /// The game's request, unless Esc freed the cursor (#576).
    fn cursor(&self, game: &GameWorld) -> CursorState {
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
        let raw_input = self.egui_winit.take_egui_input(&shell.window);
        self.egui_ctx.begin_pass(raw_input);

        self.editor_ui.preview_texture_id = self.preview_texture_id;
        let interaction = self.draw_dashboard(shell, game);
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

/// Boot the editor on the seeded default scene and run until the window closes.
pub fn launch() {
    println!("[Engine] Starting rusty 3D engine...");
    boot::seed_project_workspace();
    let scene_path = crate::scene::DEFAULT_SCENE_PATH.to_string();
    let game = boot::load_game(&scene_path);
    // Bound: the File → Build Settings window and `Application.Set*` write the file.
    let opened = game
        .resources
        .application
        .borrow_mut()
        .open(BUILD_SETTINGS_PATH);
    if let Err(err) = opened {
        game.console().borrow_mut().error(err);
    }

    let launch = Launch {
        title: "Rusty 3D Game Engine & Editor".to_string(),
        video_defaults: VideoSettings::default(),
        frontend: |shell: &Shell, game: &GameWorld| EditorFrontend::new(shell, game, scene_path),
    };
    super::run(game, launch);
}
