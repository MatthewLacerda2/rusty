#![cfg(feature = "editor")]
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
mod frame_work;
pub mod header;
pub mod hierarchy;
pub mod inspector;
mod menu_create;
pub mod project_picker;
pub mod theme;
pub use frame_work::Paint;
pub mod viewport;
pub mod widgets;

use std::cell::RefCell;
use std::rc::Rc;

use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

pub use inspector::preview::{FramedOrbit, PreviewCache, PreviewMesh, PreviewRequest};

#[cfg(feature = "dev")]
pub use dev_state::DevEditorState;
pub use viewport::{ViewportInteraction, ViewportTab};

pub enum InspectorTarget {
    Entity(u32),
    Asset(String),
    SceneSettings,
}

pub struct EditorUi {
    pub selected_entity_id: Option<u32>,
    pub selected_asset_path: Option<String>,
    scene_path: Rc<RefCell<Option<String>>>,
    pub is_dirty: bool,

    pub asset_script_content: String,

    pub show_about: bool,
    pub show_build_settings: bool,
    pub build_settings_draft: Option<rusty_core::application::BuildSettings>,

    pub hierarchy_open: bool,
    pub inspector_open: bool,
    pub bottom_open: bool,

    pub theme: theme::Theme,
    fonts_installed: bool,

    pub quality_presets: Vec<String>,
    pub quality_preset: Option<String>,
    pub quality_request: Option<String>,
    /// The speaker mode shown in Config, mirrored from the audio maestro.
    pub speaker_mode: rusty_audio::SpeakerMode,
    pub speaker_mode_request: Option<rusty_audio::SpeakerMode>,

    pub viewport_tab: ViewportTab,
    pub viewport_image_size: egui::Vec2,
    pub ui_textures: std::rc::Rc<rusty_ui::TextureSizes>,
    pub gizmo_drag: Option<viewport::gizmo::GizmoDrag>,
    /// Which gizmos both tabs draw: their strips' toggles, and `Editor.SetGizmo`.
    pub gizmos: rusty_core::application::SceneGizmos,
    pub game_view: rusty_core::application::GameViewSetting,
    pub stats: Option<viewport::stats::Readout>,
    pub rect_drag: Option<viewport::rect_tool::RectDrag>,
    pub scene_view: viewport::scene_nav::SceneView,

    pub preview_tab_active: bool,
    pub preview_mesh: PreviewMesh,
    pub preview_orbit: FramedOrbit,
    pub preview_cache: Option<PreviewCache>,
    pub preview_request: Option<PreviewRequest>,
    pub preview_texture_id: Option<egui::TextureId>,
    pub material_delete: inspector::components::material::MaterialDelete,
    /// An Undo or Redo was asked this frame: its writes are a step of their own.
    pub history_stepped: bool,

    #[cfg(feature = "dev")]
    pub dev: DevEditorState,
}

impl EditorUi {
    /// `scene_path` is the script runtime's own cell (`ScriptManager::scene_path_cell`),
    /// so the editor and `Scene.Save()` share one value.
    pub fn new(scene_path: Rc<RefCell<Option<String>>>) -> Self {
        Self {
            selected_entity_id: None,
            selected_asset_path: None,
            scene_path,
            is_dirty: false,

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
            speaker_mode: rusty_audio::SpeakerMode::default(),
            speaker_mode_request: None,
            viewport_tab: ViewportTab::Scene,
            viewport_image_size: egui::Vec2::ZERO,
            ui_textures: Default::default(),
            gizmo_drag: None,
            gizmos: Default::default(),
            game_view: Default::default(),
            stats: None,
            rect_drag: None,
            scene_view: Default::default(),

            preview_tab_active: false,
            preview_mesh: PreviewMesh::default(),
            preview_orbit: FramedOrbit::default(),
            preview_cache: None,
            preview_request: None,
            preview_texture_id: None,
            material_delete: Default::default(),
            history_stepped: false,

            #[cfg(feature = "dev")]
            dev: DevEditorState::default(),
        }
    }

    pub fn inspector_target(&self) -> InspectorTarget {
        if let Some(id) = self.selected_entity_id {
            InspectorTarget::Entity(id)
        } else if let Some(path) = &self.selected_asset_path {
            InspectorTarget::Asset(path.clone())
        } else {
            InspectorTarget::SceneSettings
        }
    }

    fn apply_theme(&mut self, ctx: &egui::Context, playing: bool) {
        if !self.fonts_installed {
            theme::Theme::install_fonts(ctx);
            self.fonts_installed = true;
        }
        let base = theme::Theme::dark();
        self.theme = if playing { base.for_play_mode() } else { base };
        self.theme.apply(ctx);
    }

    #[expect(clippy::too_many_arguments, reason = "distinct mutable borrows")]
    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        scene: &mut Scene,
        console: &mut ConsoleLogs,
        nav: &mut NavigationGraph,
        is_playing: &mut bool,
        viewport_texture: Option<egui::TextureId>,
    ) -> ViewportInteraction {
        self.apply_theme(ctx, *is_playing);
        self.preview_request = None;

        // A despawned id leaves the selection; ids are not reused within a scene, and a
        // scene swap clears the selection.
        self.selected_entity_id = self
            .selected_entity_id
            .filter(|&id| scene.world.contains(id));
        scene.selected_entity_id = self.selected_entity_id;

        let ui = &mut theme::chrome::root_ui(ctx);
        header::draw(self, ui, scene, console, is_playing);
        // After the header, inside the pass: egui drops a delayed repaint asked
        // before `begin_pass`, and the header reads a bake's refusal first.
        self.frame_work(ctx, scene, console, nav, *is_playing, Paint::Drawn);

        if *is_playing {
            #[cfg(feature = "dev")]
            if self.dev.show_play_console {
                bottom_panel::draw_play_console(self, ctx, console);
            }
        } else {
            hierarchy::draw(self, ui, scene);
            inspector::draw(self, ui, scene, console, nav);
            bottom_panel::draw(self, ui, console);
        }

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

#[cfg(test)]
#[path = "play_shortcut_tests.rs"]
mod play_shortcut_tests;
