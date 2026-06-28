//! src/editor/viewport.rs — the Unity-style Scene/Game viewport panel (#183).
//!
//! The 3D scene is rendered to an offscreen texture (`Renderer::viewport_view`) and
//! shown here inside an `egui::Image` in the central panel, with two tabs over it:
//! **Scene** (the free-fly editor camera, with click-to-select + the move gizmo) and
//! **Game** (the active `CameraComponent`'s view — what the player sees). The panel
//! owns no GPU state; `main.rs` registers the texture and feeds the [`egui::TextureId`]
//! in, and reads the returned [`ViewportInteraction`] back to drive picking/gizmo.

pub mod gizmo;
pub mod pick;

/// Which view the viewport tab strip is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewportTab {
    /// Free-fly editor camera with gizmos + selection overlays.
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
}

/// Draw the central viewport panel: the Scene/Game tab strip and the scene image.
/// `texture` is the registered offscreen scene target (`None` before the first frame
/// renders one — a placeholder is shown instead). Returns the interaction for the
/// Scene tab so the caller can pick/drag; the Game tab is view-only.
pub fn draw(
    editor: &mut crate::editor::EditorUi,
    ctx: &egui::Context,
    texture: Option<egui::TextureId>,
) -> ViewportInteraction {
    let t = editor.theme;
    let mut tab = editor.viewport_tab;

    let response = egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(t.bg_tier0))
        .show(ctx, |ui| {
            draw_tab_strip(ui, &mut tab, t);
            draw_image(ui, texture, t)
        })
        .inner;

    editor.viewport_tab = tab;
    let interaction = build_interaction(tab, &response);
    // Record the image rect's pixel size so the front-end resizes the offscreen
    // target to match (points scaled by the egui pixels-per-point).
    editor.viewport_image_size = response.rect.size();
    interaction
}

/// The tab strip: Scene / Game selectors. Mirrors Unity's viewport tabs.
fn draw_tab_strip(ui: &mut egui::Ui, tab: &mut ViewportTab, t: crate::editor::theme::Theme) {
    egui::Frame::none()
        .fill(t.bg_tier1)
        .inner_margin(t.space_xs)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(*tab == ViewportTab::Scene, "Scene")
                    .clicked()
                {
                    *tab = ViewportTab::Scene;
                }
                if ui
                    .selectable_label(*tab == ViewportTab::Game, "Game")
                    .clicked()
                {
                    *tab = ViewportTab::Game;
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

/// Translate the image's egui response into the front-end-facing interaction, with
/// pointer positions made relative to the image's top-left.
fn build_interaction(tab: ViewportTab, response: &egui::Response) -> ViewportInteraction {
    let origin = response.rect.min;
    let hover_local = response
        .hover_pos()
        .map(|p| p - origin)
        .or_else(|| response.interact_pointer_pos().map(|p| p - origin));
    ViewportInteraction {
        tab,
        size: response.rect.size(),
        hover_local,
        clicked: response.clicked(),
        dragging: response.dragged(),
        drag_delta: response.drag_delta(),
        drag_started: response.drag_started(),
    }
}
