use std::fs;

pub mod bottom_panel;
pub mod header;
pub mod hierarchy;
pub mod inspector;
mod inspector_add;
mod inspector_camera;
mod inspector_gameplay;
mod inspector_render;
mod inspector_transform;
pub mod inspectors;

use crate::core::scene::Scene;
use crate::navigation::NavigationGraph;
use crate::scripting::ConsoleLogs;

pub struct EditorUi {
    pub selected_entity_id: Option<u32>,
    pub selected_asset_path: Option<String>,
    /// The scene file the editor is currently editing. Save writes back HERE;
    /// double-clicking a `.scene` in the assets browser loads it and sets this.
    pub current_scene_path: Option<String>,
    pub current_dir: String,
    pub is_dirty: bool,
    new_entity_name: String,
    new_entity_type: String,
    new_script_path: String,
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

    /// Live Lua REPL input line (dev builds only). The editor only collects the
    /// submitted text here; the front-end (main.rs) drains `pending_repl` and runs
    /// it through the single `dev::console` evaluator against the live runtime.
    #[cfg(feature = "dev")]
    pub repl_input: crate::dev::console::ReplInput,
    /// A line the user submitted this frame, awaiting evaluation by the front-end.
    #[cfg(feature = "dev")]
    pub pending_repl: Option<String>,
}

impl EditorUi {
    pub fn new() -> Self {
        Self {
            selected_entity_id: None,
            selected_asset_path: None,
            current_scene_path: None,
            current_dir: "project".to_string(),
            is_dirty: true,
            new_entity_name: "New Primitive".to_string(),
            new_entity_type: "Box".to_string(),
            new_script_path: "project/assets/scripts/bot.lua".to_string(),
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

            #[cfg(feature = "dev")]
            repl_input: crate::dev::console::ReplInput::new(),
            #[cfg(feature = "dev")]
            pending_repl: None,
        }
    }

    /// Set up high-end professional dark theme styling with subtle cyan/teal accents (Unreal/Blender style)
    pub fn apply_theme(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();

        let visuals = &mut style.visuals;
        visuals.dark_mode = true;
        visuals.override_text_color = Some(egui::Color32::from_rgb(228, 230, 235));

        // Professional slate-gray-blue obsidian theme
        let bg_dark = egui::Color32::from_rgb(16, 16, 22); // Main panels
        let border_color = egui::Color32::from_rgb(35, 35, 45); // Borders
        let widget_inactive = egui::Color32::from_rgb(28, 28, 36);
        let widget_hovered = egui::Color32::from_rgb(38, 38, 50);
        let widget_active = egui::Color32::from_rgb(46, 46, 62);

        let accent_cyan = egui::Color32::from_rgb(0, 229, 255);
        let accent_teal = egui::Color32::from_rgb(0, 242, 254);

        visuals.widgets.noninteractive.bg_fill = bg_dark;
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, border_color);

        // Inactive widgets
        visuals.widgets.inactive.bg_fill = widget_inactive;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, border_color);
        visuals.widgets.inactive.rounding = egui::Rounding::same(4.0);

        // Hovered widgets
        visuals.widgets.hovered.bg_fill = widget_hovered;
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.2, accent_cyan);
        visuals.widgets.hovered.rounding = egui::Rounding::same(4.0);

        // Active/Focused widgets
        visuals.widgets.active.bg_fill = widget_active;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5, accent_teal);
        visuals.widgets.active.rounding = egui::Rounding::same(4.0);

        visuals.selection.bg_fill = egui::Color32::from_rgb(0, 75, 90);

        visuals.window_rounding = egui::Rounding::same(6.0);

        // Tight spacing and clean layout parameters
        style.spacing.item_spacing = egui::vec2(6.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);

        ctx.set_style(style);
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

    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        scene: &mut Scene,
        console: &mut ConsoleLogs,
        nav: &mut NavigationGraph,
        is_playing: &mut bool,
        _fps: f32,
        _frame_time: f32,
    ) {
        self.apply_theme(ctx);
        self.scan_assets();

        // Push editor selection to scene (editor UI is the authority)
        scene.selected_entity_id = self.selected_entity_id;

        // 1. TOP HEADER PANEL (Controls engine state) — ALWAYS VISIBLE
        header::draw(self, ctx, scene, console, is_playing);

        if *is_playing {
            // The runtime is only live during play, so the REPL belongs here. Show a
            // floating console (log + input line) so you can call the API while the
            // game runs. Dev builds only — stripped from ship builds with the rest of
            // the agentic layer.
            #[cfg(feature = "dev")]
            bottom_panel::draw_play_console(self, ctx, console);
            return;
        }

        // 2. LEFT PANEL: Scene Hierarchy
        hierarchy::draw(self, ctx, scene);

        // 3. RIGHT PANEL: Properties Inspector
        inspector::draw(self, ctx, scene, console, nav);

        // 4. BOTTOM PANEL: Folder Explorer & Console Logs
        bottom_panel::draw(self, ctx, scene, console);
    }
}
