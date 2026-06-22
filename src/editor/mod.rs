use std::fs;

pub mod bottom_panel;
mod content_browser;
mod content_grid;
pub mod header;
pub mod hierarchy;
mod hierarchy_tree;
pub mod inspector;
mod inspector_add;
mod inspector_audio;
mod inspector_camera;
mod inspector_card;
mod inspector_context;
mod inspector_gameplay;
mod inspector_material;
mod inspector_particles;
mod inspector_prefab;
mod inspector_render;
mod inspector_script_fields;
mod inspector_settings;
mod inspector_transform;
pub mod inspectors;
mod menu_create;
pub mod theme;
pub mod viewport;
pub mod viewport_gizmo;
pub mod viewport_pick;

use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

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
    /// The scene file the editor is currently editing. Save writes back HERE;
    /// double-clicking a `.scene` in the assets browser loads it and sets this.
    pub current_scene_path: Option<String>,
    pub current_dir: String,
    pub is_dirty: bool,
    assets_scripts: Vec<String>,
    assets_textures: Vec<String>,

    // Custom asset inspector properties
    pub asset_image_wrap: String,
    pub asset_image_filter: String,
    pub asset_image_mipmaps: bool,
    pub asset_audio_volume: f32,
    pub asset_audio_pitch: f32,
    pub asset_audio_loop: bool,
    pub asset_audio_playing: bool,
    pub asset_audio_play_time: Option<std::time::Instant>,
    pub asset_model_scale: f32,
    pub asset_model_import_normals: bool,
    pub asset_script_content: String,
    pub active_bottom_tab: String,
    /// Content-browser name filter (the search bar above the tile grid).
    pub asset_search: String,

    /// Whether the About dialog (File-bar → About) is currently open.
    pub show_about: bool,

    /// Whether each dockable panel is expanded. Collapsing shrinks the panel to a
    /// thin rail with a caret that brings it back. Pure runtime UI state.
    pub hierarchy_open: bool,
    pub inspector_open: bool,
    pub bottom_open: bool,

    /// The active visual theme — the single source of truth for editor colors,
    /// spacing and the type scale. Panels read tokens from here (or via
    /// `theme::from_ui`) instead of hardcoding colors.
    pub theme: theme::Theme,
    /// Phosphor icon font is registered once, lazily, on the first draw.
    fonts_installed: bool,

    /// Selected post-FX scalability tier. main.rs syncs this onto the renderer
    /// each frame so Low/Medium/High gate which passes run + buffer sizes.
    pub quality_preset: crate::render::postfx::QualityPreset,

    /// Which viewport tab is showing — Scene (editor cam + gizmos) or Game (the
    /// active camera's view). Both are reachable in edit and play mode (#183).
    pub viewport_tab: ViewportTab,
    /// The scene image's last drawn size in egui points, recorded each frame so the
    /// front-end can size the offscreen render target to match the displayed rect.
    pub viewport_image_size: egui::Vec2,
    /// Live move-gizmo drag, if an axis handle is currently grabbed (#183).
    pub gizmo_drag: Option<viewport_gizmo::GizmoDrag>,

    /// Live Lua REPL input line (dev builds only). The editor only collects the
    /// submitted text here; the front-end (main.rs) drains `pending_repl` and runs
    /// it through the single `dev::console` evaluator against the live runtime.
    #[cfg(feature = "dev")]
    pub repl_input: crate::dev::console::ReplInput,
    /// A line the user submitted this frame, awaiting evaluation by the front-end.
    #[cfg(feature = "dev")]
    pub pending_repl: Option<String>,
}

impl Default for EditorUi {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorUi {
    pub fn new() -> Self {
        Self {
            selected_entity_id: None,
            selected_asset_path: None,
            current_scene_path: None,
            current_dir: "project".to_string(),
            is_dirty: true,
            assets_scripts: Vec::new(),
            assets_textures: Vec::new(),

            asset_image_wrap: "Repeat".to_string(),
            asset_image_filter: "Linear".to_string(),
            asset_image_mipmaps: true,
            asset_audio_volume: 0.8,
            asset_audio_pitch: 1.0,
            asset_audio_loop: false,
            asset_audio_playing: false,
            asset_audio_play_time: None,
            asset_model_scale: 1.0,
            asset_model_import_normals: true,
            asset_script_content: String::new(),
            active_bottom_tab: "assets".to_string(),
            asset_search: String::new(),
            show_about: false,
            hierarchy_open: true,
            inspector_open: true,
            bottom_open: true,
            theme: theme::Theme::dark(),
            fonts_installed: false,
            quality_preset: crate::render::postfx::QualityPreset::default(),
            viewport_tab: ViewportTab::Scene,
            viewport_image_size: egui::Vec2::ZERO,
            gizmo_drag: None,

            #[cfg(feature = "dev")]
            repl_input: crate::dev::console::ReplInput::new(),
            #[cfg(feature = "dev")]
            pending_repl: None,
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

    /// Apply the active theme: register the Phosphor icon font once, then push the
    /// token-derived [`egui::Style`] for this frame. See [`theme::Theme`].
    pub fn apply_theme(&mut self, ctx: &egui::Context) {
        if !self.fonts_installed {
            theme::Theme::install_fonts(ctx);
            self.fonts_installed = true;
        }
        self.theme.apply(ctx);
    }

    /// Read local assets folders to populate Asset Browser (legacy scan, kept for compatibility)
    pub fn scan_assets(&mut self) {
        self.assets_scripts.clear();
        self.assets_textures.clear();

        // Scan scripts
        if let Ok(entries) = fs::read_dir("project/assets/scripts") {
            for entry in entries.flatten() {
                if let Some(path_str) = entry.path().to_str() {
                    if path_str.ends_with(".lua") {
                        self.assets_scripts.push(path_str.replace("\\", "/"));
                    }
                }
            }
        }

        // Scan textures
        if let Ok(entries) = fs::read_dir("project/assets/textures") {
            for entry in entries.flatten() {
                if let Some(path_str) = entry.path().to_str() {
                    let path_lower = path_str.to_lowercase();
                    if path_lower.ends_with(".png") || path_lower.ends_with(".tga") {
                        self.assets_textures.push(path_str.replace("\\", "/"));
                    }
                }
            }
        }
    }

    // The editor draw entry point threads the live engine state (scene, console,
    // nav, play flag) plus the per-frame HUD metrics; these are distinct mutable
    // borrows that a parameter struct could not group without fighting the borrow
    // checker. Legacy signature — left as-is pending the App/Resources migration.
    #[allow(clippy::too_many_arguments)]
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
        self.apply_theme(ctx);
        self.scan_assets();

        // Push editor selection to scene (editor UI is the authority)
        scene.selected_entity_id = self.selected_entity_id;

        // 1. TOP HEADER PANEL (Controls engine state) — ALWAYS VISIBLE
        header::draw(self, ctx, scene, console, is_playing);

        // The side/bottom authoring panels are edit-mode only; during play the runtime
        // is live and the floating dev console takes over. The viewport stays in both
        // modes so the Scene/Game tabs are always reachable (#183).
        if *is_playing {
            #[cfg(feature = "dev")]
            bottom_panel::draw_play_console(self, ctx, console);
        } else {
            // 2. LEFT PANEL: Scene Hierarchy
            hierarchy::draw(self, ctx, scene);
            // 3. RIGHT PANEL: Properties Inspector
            inspector::draw(self, ctx, scene, console, nav);
            // 4. BOTTOM PANEL: Folder Explorer & Console Logs
            bottom_panel::draw(self, ctx, scene, console);
        }

        // 5. CENTRAL VIEWPORT: the Scene/Game tabbed scene image. Drawn last so it
        // fills the space the docked panels leave. Returns the pointer interaction the
        // front-end uses for click-to-select and the move gizmo on the Scene tab.
        viewport::draw(self, ctx, viewport_texture)
    }
}
