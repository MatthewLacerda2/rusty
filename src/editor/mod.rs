#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "platform layer, exempt from the sim's clock ban (#757): the audio preview playhead and script-schema mtimes are wall-clock UI state"
)]

pub mod bottom_panel;
pub mod build_settings;
#[cfg(feature = "dev")]
pub mod demo;
#[cfg(feature = "dev")]
mod dev_state;
pub mod header;
pub mod hierarchy;
mod hierarchy_tree;
pub mod inspector;
mod menu_create;
pub mod project_picker;
pub mod theme;
pub mod viewport;
pub mod widgets;

use std::cell::RefCell;
use std::rc::Rc;

use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

pub use inspector::preview::{OrbitState, PreviewCache, PreviewMesh, PreviewRequest};

#[cfg(feature = "dev")]
pub use dev_state::DevEditorState;
pub use viewport::{ViewportInteraction, ViewportTab};

/// What the Inspector panel is currently showing. The three modes are mutually
/// exclusive; the editor derives this from its selection state each frame so the
/// inspector dispatches on an explicit target rather than an implicit fall-through.
pub enum InspectorTarget {
    Entity(u32),
    Asset(String),
    SceneSettings,
}

pub struct EditorUi {
    pub selected_entity_id: Option<u32>,
    pub selected_asset_path: Option<String>,
    /// The scene file the editor is currently editing: Save writes back here. One
    /// value with `Scene.Save()`'s write-back target (#1366): the shell hands the
    /// editor the script runtime's own cell ([`EditorUi::new`]), so
    /// File ▸ Open/New, saving to a scene asset, `Scene.Load` and `Scene.Save(path)`
    /// all change it, and nothing reconciles two copies.
    scene_path: Rc<RefCell<Option<String>>>,
    pub is_dirty: bool,

    // Custom asset inspector properties
    pub asset_script_content: String,

    /// Whether the About dialog (File-bar → About) is currently open.
    pub show_about: bool,
    /// Whether the File → Build Settings window is open, and its unsaved draft (#431).
    pub show_build_settings: bool,
    pub build_settings_draft: Option<crate::core::application::BuildSettings>,

    /// Whether each dockable panel is expanded. Collapsing shrinks the panel to a
    /// thin rail with a caret that brings it back. Pure runtime UI state.
    pub hierarchy_open: bool,
    pub inspector_open: bool,
    pub bottom_open: bool,

    /// The active visual theme — the single source of truth for editor colors,
    /// spacing and the type scale. Panels read tokens from here (or via
    /// `theme::from_ui`) instead of hardcoding colors.
    pub theme: theme::Theme,
    /// The editor fonts (Inter, JetBrains Mono, Phosphor) are registered once,
    /// lazily, on the first draw.
    fonts_installed: bool,

    /// The project's quality presets' names and the running one (#1522), mirrored
    /// from `Application` each frame for the Config menu.
    pub quality_presets: Vec<String>,
    pub quality_preset: Option<String>,
    /// A preset picked in Config this frame, for the shell to select.
    pub quality_request: Option<String>,
    /// The speaker mode shown in Config (#546), mirrored from the audio maestro.
    pub speaker_mode: crate::audio::SpeakerMode,
    /// A speaker mode picked in Config this frame, for the shell to apply.
    pub speaker_mode_request: Option<crate::audio::SpeakerMode>,

    /// Which viewport tab is showing — Scene (editor cam + gizmos) or Game (the
    /// active camera's view). Both are reachable in edit and play mode (#183).
    pub viewport_tab: ViewportTab,
    /// The scene image's last drawn size in egui points, recorded each frame so the
    /// front-end can size the offscreen render target to match the displayed rect.
    pub viewport_image_size: egui::Vec2,
    /// The texture sizes the UI lays out with (#1221): the sim's own table
    /// (`ScreenSize::textures`), handed over each frame by the shell, so the
    /// overlay and the inspector measure Images as the game does.
    pub ui_textures: std::rc::Rc<crate::ui::TextureSizes>,
    /// Live move-gizmo drag, if an axis handle is currently grabbed (#183).
    pub gizmo_drag: Option<viewport::gizmo::GizmoDrag>,
    /// The Scene tab's UI overlay: screen canvases drawn, the rect tool live (#423).
    pub ui_overlay: bool,
    /// Live rect-tool drag on a UI element's handle (#423).
    pub rect_drag: Option<viewport::rect_tool::RectDrag>,
    /// The Scene tab's own camera and what the pointer is doing with it (#745).
    pub scene_view: viewport::scene_nav::SceneView,

    /// Whether the Inspector's asset/material cards show "Details" or "Preview"
    /// (#352). One shared flag: entity and asset selection are mutually exclusive,
    /// so only one card is ever showing it at a time.
    pub preview_tab_active: bool,
    /// The Preview tab's mesh toggle (sphere/cube/Suzanne).
    pub preview_mesh: PreviewMesh,
    /// The Preview tab's orbit-camera state, persisted across frames.
    pub preview_orbit: OrbitState,
    /// The isolated preview scene, rebuilt only when the subject/mesh choice changes.
    pub preview_cache: Option<PreviewCache>,
    /// This frame's render request for `main.rs`'s `render_preview_scene`; `None`
    /// when the Preview tab isn't showing (skips the extra offscreen render).
    pub preview_request: Option<PreviewRequest>,
    /// Stable egui texture id for the offscreen preview target (#352), set by
    /// `main.rs` before `draw` runs — mirrors `Frontend::viewport_texture_id`.
    pub preview_texture_id: Option<egui::TextureId>,
    /// The Material card's delete confirm and its one *Undo* step (#1576).
    pub material_delete: inspector::components::material::MaterialDelete,

    /// The dev-only editor state: the REPL line and the background lightmap bake.
    #[cfg(feature = "dev")]
    pub dev: DevEditorState,
}

impl EditorUi {
    /// An editor whose current scene file is `scene_path`: the script runtime's own
    /// cell (`ScriptManager::scene_path_cell`), so the editor and `Scene.Save()` share
    /// one value (#1366). A constructor argument, so no caller can forget it.
    pub fn new(scene_path: Rc<RefCell<Option<String>>>) -> Self {
        Self {
            selected_entity_id: None,
            selected_asset_path: None,
            scene_path,
            is_dirty: true,

            asset_script_content: String::new(),
            show_about: false,
            show_build_settings: false,
            build_settings_draft: None,
            hierarchy_open: true,
            inspector_open: true,
            bottom_open: true,
            theme: theme::Theme::dark(),
            fonts_installed: false,
            quality_presets: Vec::new(),
            quality_preset: None,
            quality_request: None,
            speaker_mode: crate::audio::SpeakerMode::default(),
            speaker_mode_request: None,
            viewport_tab: ViewportTab::Scene,
            viewport_image_size: egui::Vec2::ZERO,
            ui_textures: Default::default(),
            gizmo_drag: None,
            ui_overlay: true,
            rect_drag: None,
            scene_view: Default::default(),

            preview_tab_active: false,
            preview_mesh: PreviewMesh::default(),
            preview_orbit: OrbitState::default(),
            preview_cache: None,
            preview_request: None,
            preview_texture_id: None,
            material_delete: Default::default(),

            #[cfg(feature = "dev")]
            dev: DevEditorState::default(),
        }
    }

    /// The inspector's current target, derived from the (mutually exclusive)
    /// selection state: a selected entity wins, else a selected asset, else the
    /// scene settings.
    pub fn inspector_target(&self) -> InspectorTarget {
        if let Some(id) = self.selected_entity_id {
            InspectorTarget::Entity(id)
        } else if let Some(path) = &self.selected_asset_path {
            InspectorTarget::Asset(path.clone())
        } else {
            InspectorTarget::SceneSettings
        }
    }

    /// Apply the active theme: register the editor fonts once, then push the
    /// token-derived [`egui::Style`] for this frame — tinted toward the accent while
    /// `playing`, as Unity tints its editor in Play mode. See [`theme::Theme`].
    fn apply_theme(&mut self, ctx: &egui::Context, playing: bool) {
        if !self.fonts_installed {
            theme::Theme::install_fonts(ctx);
            self.fonts_installed = true;
        }
        let base = theme::Theme::dark();
        self.theme = if playing { base.for_play_mode() } else { base };
        self.theme.apply(ctx);
    }

    // The live engine state (scene, console, nav, play flag) plus the frame's HUD
    // metrics: distinct mutable borrows. Legacy signature, pending App/Resources.
    #[expect(clippy::too_many_arguments, reason = "distinct mutable borrows")]
    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        scene: &mut Scene,
        console: &mut ConsoleLogs,
        nav: &mut NavigationGraph,
        is_playing: &mut bool,
        _fps: f32,
        _frame_time: f32,
        viewport_texture: Option<egui::TextureId>,
    ) -> ViewportInteraction {
        self.apply_theme(ctx, *is_playing);
        // Set by the Preview tab if it shows (#352); `None` skips its offscreen render.
        self.preview_request = None;

        // The editor picks the selection and pushes it to the scene; the World says
        // what may stay selected. An entity despawned since the last frame (a
        // script's `Scene.DestroyEntity`, a re-import's bones) leaves it (#1264).
        // Ids are never reused within a scene, and a scene swap clears it.
        self.selected_entity_id = self
            .selected_entity_id
            .filter(|&id| scene.world.contains(id));
        scene.selected_entity_id = self.selected_entity_id;

        let ui = &mut theme::chrome::root_ui(ctx);
        // 1. TOP HEADER PANEL (Controls engine state) — ALWAYS VISIBLE
        header::draw(self, ui, scene, console, is_playing);

        // The side/bottom authoring panels are edit-mode only; during play the runtime
        // is live and the floating dev console takes over. The viewport stays in both
        // modes so the Scene/Game tabs are always reachable (#183).
        if *is_playing {
            // Off by default and toggled from the header (#1544); hidden, nothing is
            // drawn and no field can take focus. Logs are collected either way.
            #[cfg(feature = "dev")]
            if self.dev.show_play_console {
                bottom_panel::draw_play_console(self, ctx, console);
            }
        } else {
            // One diffing navmesh sync per edit-mode frame catches every edit since
            // the last one, whoever made it: a card, the gizmo, the console, MCP
            // (#992). It rebakes only changed cells; play mode has `rebake_nav`.
            nav.sync(scene);
            // 2. LEFT PANEL: Scene Hierarchy
            hierarchy::draw(self, ui, scene);
            // 3. RIGHT PANEL: Properties Inspector
            inspector::draw(self, ui, scene, console, nav);
            // 4. BOTTOM PANEL: Console
            bottom_panel::draw(self, ui, console);
        }

        // 5. CENTRAL VIEWPORT: the Scene/Game tabbed scene image. Drawn last so it
        // fills the space the docked panels leave. Returns the pointer interaction the
        // front-end uses for click-to-select and the move gizmo on the Scene tab.
        viewport::draw(self, ui, viewport_texture, scene)
    }
}

#[cfg(test)]
#[path = "nav_sync_tests.rs"]
mod nav_sync_tests;

#[cfg(test)]
#[path = "selection_tests.rs"]
mod selection_tests;

#[cfg(test)]
#[path = "undo_tests.rs"]
mod undo_tests;
