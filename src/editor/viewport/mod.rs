//! src/editor/viewport.rs — the Unity-style Scene/Game viewport panel (#183).
//!
//! The 3D scene is rendered to an offscreen texture (`Renderer::viewport_view`) and
//! shown here inside an `egui::Image` in the central panel, with two tabs over it:
//! **Scene** (the editor's own camera, [`scene_camera`], navigated with Unity's
//! controls by [`scene_nav`] — plus click-to-select and the move gizmo) and **Game**
//! (the active `CameraComponent`'s view — what the player sees). The panel
//! owns no GPU state; the shell's editor frontend registers the texture and feeds the [`egui::TextureId`]
//! in, and reads the returned [`ViewportInteraction`] back to drive picking/gizmo.
//! The Scene tab's **UI** toggle draws the screen-space canvases over the scene and
//! enables the rect tool on them (`rect_tool`, #423).

pub mod gizmo;
pub mod pick;
pub mod rect_tool;
pub mod scene_camera;
pub mod scene_nav;

use egui_phosphor::regular as icon;

use crate::editor::theme::chrome;

/// Which view the viewport tab strip is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewportTab {
    /// The editor's own Scene camera with gizmos + selection overlays.
    Scene,
    /// The active `CameraComponent`'s view — what ships.
    Game,
}

/// Per-frame report of the viewport's geometry and pointer interaction, handed back
/// to the front-end so it can size the offscreen target and run picking/gizmo against
/// the same rect the image was drawn in. Positions are in viewport-local *points*
/// with origin at the image's top-left.
pub struct ViewportInteraction {
    pub tab: ViewportTab,
    /// The image rect's top-left in window points — where the game view sits, so the
    /// shell can map the OS pointer into game-view pixels (#416).
    pub origin: egui::Pos2,
    /// The image rect's size in points (egui logical units).
    pub size: egui::Vec2,
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
    /// A pointer button went down anywhere in the window this frame — with
    /// `hover_local` it tells a click into the Game view from a click elsewhere.
    pub pointer_pressed: bool,
}

/// Draw the central viewport panel: the Scene/Game tab strip and the scene image.
/// `texture` is the registered offscreen scene target (`None` before the first frame
/// renders one — a placeholder is shown instead). Returns the interaction for the
/// Scene tab so the caller can pick/drag; the Game tab is view-only.
pub fn draw(
    editor: &mut crate::editor::EditorUi,
    ui: &mut egui::Ui,
    texture: Option<egui::TextureId>,
    scene: &crate::scene::Scene,
) -> ViewportInteraction {
    let t = editor.theme;
    let mut tab = editor.viewport_tab;
    let mut overlay = editor.ui_overlay;

    let response = egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(t.bg_tier0))
        .show(ui, |ui| {
            draw_tab_strip(ui, &mut tab, &mut overlay, t);
            let response = draw_image(ui, texture, t);
            if tab == ViewportTab::Scene {
                navigate(editor, &response, scene);
            } else {
                editor.scene_view.looking = false;
            }
            if tab == ViewportTab::Scene && overlay {
                let frame = rect_tool::OverlayFrame {
                    size: glam::Vec2::new(response.rect.width(), response.rect.height()),
                    pixels_per_point: ui.ctx().pixels_per_point(),
                };
                let painter = ui.painter_at(response.rect);
                let selected = editor.selected_entity_id;
                let world = &scene.world;
                rect_tool::overlay::paint(&painter, response.rect.min, &frame, world, selected, &t);
            }
            response
        })
        .inner;

    editor.viewport_tab = tab;
    editor.ui_overlay = overlay;
    let interaction = build_interaction(tab, &response, editor.scene_view.navigating);
    // Record the image rect's pixel size so the front-end resizes the offscreen
    // target to match (points scaled by the egui pixels-per-point).
    editor.viewport_image_size = response.rect.size();
    interaction
}

/// The tab strip: Scene / Game selectors (Unity's viewport tabs), and on the Scene
/// tab the UI overlay toggle.
fn draw_tab_strip(
    ui: &mut egui::Ui,
    tab: &mut ViewportTab,
    overlay: &mut bool,
    t: crate::editor::theme::Theme,
) {
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
                if *tab == ViewportTab::Scene {
                    ui.separator();
                    ui.toggle_value(overlay, "UI").on_hover_text(
                        "Draw the screen-space canvases and edit them with the rect tool",
                    );
                }
            });
        });
}

/// Draw the scene image filling the remaining space, sensing clicks + drags. Before
/// the first offscreen frame exists, a neutral fill stands in.
fn draw_image(
    ui: &mut egui::Ui,
    texture: Option<egui::TextureId>,
    t: crate::editor::theme::Theme,
) -> egui::Response {
    let size = ui.available_size();
    match texture {
        Some(id) => ui.add(
            egui::Image::new((id, size))
                .sense(egui::Sense::click_and_drag())
                .maintain_aspect_ratio(false),
        ),
        None => {
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0.0, t.bg_tier0);
            response
        }
    }
}

/// Drive the Scene camera from this frame's input on the image (#745): the held
/// gesture, scroll, and F to frame the selection while hovered.
fn navigate(
    editor: &mut crate::editor::EditorUi,
    response: &egui::Response,
    scene: &crate::scene::Scene,
) {
    let nav = scene_nav::read(response, response.is_pointer_button_down_on());
    if nav.gesture.is_some() {
        // The Scene view takes the keyboard, so WASD never types into a field.
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

/// Translate the image's egui response into the front-end-facing interaction, with
/// pointer positions made relative to the image's top-left. While the Scene camera
/// navigates (a gesture held, or Alt down), LMB is the camera's: no pick, no gizmo.
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
        hover_local,
        clicked: tools && response.clicked(),
        dragging: tools && response.dragged_by(egui::PointerButton::Primary),
        drag_delta: response.drag_delta(),
        drag_started: tools && response.drag_started_by(egui::PointerButton::Primary),
        pointer_pressed: response.ctx.input(|i| i.pointer.any_pressed()),
    }
}
