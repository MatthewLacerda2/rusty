//! crates/editor/src/viewport/ — the Unity-style Scene/Game viewport panel.

pub mod game_size;
pub mod gizmo;
pub mod pick;
pub mod rect_tool;
pub mod scene_camera;
pub mod scene_nav;
pub mod stats;

use egui_phosphor::regular as icon;

use crate::theme::chrome;
use rusty_core::application::Gizmo;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewportTab {
    Scene,
    /// The active `CameraComponent`'s view — what ships.
    Game,
}

pub struct ViewportInteraction {
    pub tab: ViewportTab,
    pub origin: egui::Pos2,
    /// The image rect's size in points (egui logical units).
    pub size: egui::Vec2,
    pub render: Option<(u32, u32)>,
    pub screen: Option<(u32, u32)>,
    /// Pointer position relative to the image's top-left, if hovering.
    pub hover_local: Option<egui::Vec2>,
    /// A click was released inside the image this frame (select / gizmo grab end).
    pub clicked: bool,
    /// The primary button is held and dragging this frame.
    pub dragging: bool,
    /// Pointer drag delta this frame, in points.
    pub drag_delta: egui::Vec2,
    /// The drag started this frame (pointer pressed inside the image).
    pub drag_started: bool,
    pub pointer_pressed: bool,
}

/// Draw the central viewport panel: the Scene/Game tab strip and the scene image.
pub fn draw(
    editor: &mut crate::EditorUi,
    ui: &mut egui::Ui,
    texture: Option<egui::TextureId>,
    scene: &rusty_scene::Scene,
) -> ViewportInteraction {
    let t = editor.theme;
    let mut fit = None;

    let response = egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(t.bg_tier0))
        .show(ui, |ui| {
            draw_tab_strip(ui, editor, t);
            let (tab, gizmos) = (editor.viewport_tab, editor.gizmos);
            fit = game_view_fit(editor, ui);
            let boxed = fit.filter(|_| tab == ViewportTab::Game);
            let response = draw_image(ui, texture, t, boxed);
            if tab == ViewportTab::Scene {
                navigate(editor, &response, scene);
            } else {
                editor.scene_view.looking = false;
            }
            if let Some(readout) = editor.stats.as_ref().filter(|_| tab == ViewportTab::Game) {
                stats::paint(ui, response.rect, &readout.lines(), t);
            }
            if tab == ViewportTab::Scene && gizmos.on(Gizmo::Ui) {
                let size = glam::Vec2::new(response.rect.width(), response.rect.height());
                let ppp = ui.ctx().pixels_per_point();
                let frame = rect_tool::OverlayFrame::new(size, ppp, fit, &editor.ui_textures);
                let painter = ui.painter_at(response.rect);
                let selected = editor.selected_entity_id;
                let world = &scene.world;
                rect_tool::overlay::paint(&painter, response.rect.min, &frame, world, selected, &t);
            }
            response
        })
        .inner;

    let tab = editor.viewport_tab;
    let mut interaction = build_interaction(tab, &response, editor.scene_view.navigating);
    interaction.render = fit.filter(|_| tab == ViewportTab::Game);
    interaction.screen = fit;
    editor.viewport_image_size = response.rect.size();
    interaction
}

fn draw_tab_strip(ui: &mut egui::Ui, editor: &mut crate::EditorUi, t: crate::theme::Theme) {
    #[cfg(feature = "dev")]
    let demo_open = editor.demo_open();
    // lint: allow(gpu-default-only) ship stub: no demo exists without dev, and `false` is what a dev build reads with none open
    #[cfg(not(feature = "dev"))]
    let demo_open = false;
    let crate::EditorUi {
        viewport_tab: tab,
        gizmos,
        game_view,
        ..
    } = editor;
    egui::Frame::NONE
        .fill(t.bg_tier1)
        .inner_margin(egui::vec2(t.space_sm, t.space_xs + 1.0))
        .stroke(egui::Stroke::new(1.0, t.border))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let scene = format!("{}  Scene", icon::CUBE_FOCUS);
                if chrome::tab(ui, &t, *tab == ViewportTab::Scene, &scene).clicked() {
                    *tab = ViewportTab::Scene;
                }
                let game = format!("{}  Game", icon::GAME_CONTROLLER);
                if chrome::tab(ui, &t, *tab == ViewportTab::Game, &game).clicked() {
                    *tab = ViewportTab::Game;
                }
                let game_tab = *tab == ViewportTab::Game;
                ui.separator();
                let rows = Gizmo::ALL.iter().filter(|g| g.row().shown);
                for &gizmo in rows.filter(|g| g.row().game == game_tab) {
                    let row = gizmo.row();
                    ui.toggle_value(gizmos.flag_mut(gizmo), row.label)
                        .on_hover_text(row.hover);
                }
                if game_tab {
                    ui.separator();
                    game_size::dropdown(ui, game_view, !demo_open);
                } else {
                    ui.label(egui::RichText::new(icon::QUESTION).color(t.text_secondary))
                        .on_hover_ui(navigation_help);
                }
            });
        });
}

fn game_view_fit(editor: &mut crate::EditorUi, ui: &mut egui::Ui) -> Option<(u32, u32)> {
    #[cfg(feature = "dev")]
    if let Some(demo) = editor.dev.demo.as_mut() {
        if editor.viewport_tab == ViewportTab::Game {
            crate::demo::timeline::draw(ui, demo, &editor.theme);
        }
        return Some(demo.size);
    }
    let ppp = ui.ctx().pixels_per_point();
    game_size::fit(editor.game_view, ui.available_size(), ppp)
}

/// The Scene view's navigation bindings, the tooltip of the tab strip's `?`.
fn navigation_help(ui: &mut egui::Ui) {
    ui.strong("Scene view navigation");
    egui::Grid::new("scene_nav_help").show(ui, |ui| {
        for (input, gesture) in scene_nav::BINDINGS {
            ui.label(*input);
            ui.label(*gesture);
            ui.end_row();
        }
    });
}

fn draw_image(
    ui: &mut egui::Ui,
    texture: Option<egui::TextureId>,
    t: crate::theme::Theme,
    fit: Option<(u32, u32)>,
) -> egui::Response {
    let size = ui.available_size();
    let Some(id) = texture else {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        ui.painter().rect_filled(rect, 0.0, t.bg_tier0);
        return response;
    };
    let Some((width, height)) = fit else {
        let image = egui::Image::new((id, size)).sense(egui::Sense::click_and_drag());
        return ui.add(image.maintain_aspect_ratio(false));
    };
    let (panel, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let rect =
        egui::Rect::from_center_size(panel.center(), game_size::letterbox(size, (width, height)));
    let painter = ui.painter();
    painter.rect_filled(panel, 0.0, egui::Color32::BLACK);
    let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
    painter.image(id, rect, uv, egui::Color32::WHITE);
    ui.interact(
        rect,
        ui.id().with("letterboxed"),
        egui::Sense::click_and_drag(),
    )
}

fn navigate(editor: &mut crate::EditorUi, response: &egui::Response, scene: &rusty_scene::Scene) {
    let nav = scene_nav::read(response, response.is_pointer_button_down_on());
    if nav.keeps_moving() {
        response.ctx.request_repaint();
    }
    if nav.gesture.is_some() {
        response.request_focus();
    }
    scene_nav::apply(&mut editor.scene_view.camera, &nav);
    editor.scene_view.navigating = nav.gesture.is_some() || response.ctx.input(|i| i.modifiers.alt);
    editor.scene_view.looking = nav.gesture == Some(scene_nav::Gesture::Look);
    let frame_key = response.ctx.input(|i| i.key_pressed(egui::Key::F));
    if frame_key && response.hovered() && !response.ctx.text_edit_focused() {
        let bounds = editor
            .selected_entity_id
            .and_then(|id| scene_nav::selection_bounds(scene, id));
        if let Some((center, radius)) = bounds {
            editor.scene_view.camera.frame(center, radius);
        }
    }
}

fn build_interaction(
    tab: ViewportTab,
    response: &egui::Response,
    navigating: bool,
) -> ViewportInteraction {
    let tools = !(navigating && tab == ViewportTab::Scene);
    let origin = response.rect.min;
    let hover_local = response
        .hover_pos()
        .map(|p| p - origin)
        .or_else(|| response.interact_pointer_pos().map(|p| p - origin));
    ViewportInteraction {
        tab,
        origin,
        size: response.rect.size(),
        render: None,
        screen: None,
        hover_local,
        clicked: tools && response.clicked(),
        dragging: tools && response.dragged_by(egui::PointerButton::Primary),
        drag_delta: response.drag_delta(),
        drag_started: tools && response.drag_started_by(egui::PointerButton::Primary),
        pointer_pressed: response.ctx.input(|i| i.pointer.any_pressed()),
    }
}
