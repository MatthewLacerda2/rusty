//! crates/shell/src/editor/capture/viewport.rs — the capture's Scene/Game viewport.

use std::rc::Rc;

use rusty_app::GameWorld;
use rusty_core::application::Gizmo;
use rusty_dev::capture::CaptureHost;
use rusty_editor::{EditorUi, ViewportTab};

/// `shell::editor`'s `render_viewport_scene` against the capture host: draw, then
/// bind; the size in pixels of the colour target it drew into, `None` when no
/// frame was rendered (an empty viewport, no adapter).
pub(super) fn render_viewport(
    host: &mut CaptureHost,
    egui_renderer: &mut egui_wgpu::Renderer,
    game: &GameWorld,
    ui: &EditorUi,
    drawn: rusty_editor::ViewportInteraction,
    texture_id: &mut Option<egui::TextureId>,
) -> Option<(u32, u32)> {
    let scene_tab = ui.viewport_tab == ViewportTab::Scene;
    // The Game view's own size, letterboxed: a demo's (#1598) or the dropdown's (#2207).
    let size = drawn.size;
    let panel = (size.x.round() as u32, size.y.round() as u32);
    let (px, py) = drawn.render.unwrap_or(panel);
    if px == 0 || py == 0 {
        return None;
    }
    if ui.demo_size().is_none() {
        // The Game view's own size behind the Scene tab too, as the window's (#2207).
        let (sx, sy) = drawn.screen.unwrap_or(panel);
        game.resources.screen.borrow_mut().set_game_view(sx, sy);
    }
    // The settings first: the Game view's scene renders at the render scale (#1498).
    let renderer = host.renderer(px, py)?;
    let display_scale = game.resources.screen.borrow().display_scale();
    renderer.apply_graphics(
        *game.script_manager().graphics_cell().borrow(),
        display_scale,
    );
    let scale = renderer.render_scale(scene_tab);
    let (renderer, view) = host.frame(px, py, scale)?;
    let target = view.color_target_view()?;
    let scene = game.scene().borrow();
    view.gizmos = ui.gizmos;
    if scene_tab {
        super::super::navmesh::hand(renderer, &view.gizmos, &game.resources.nav.borrow());
    }
    view.ui.screen_in_editor = scene_tab && ui.gizmos.on(Gizmo::Ui);
    view.ui.screen_box = rusty_editor::viewport::game_size::overlay_box(&drawn, (px, py));
    view.game_gizmos = !scene_tab && ui.gizmos.on(Gizmo::GameGizmos);
    let camera = super::super::viewport::viewport_camera(ui, game, &scene, ui.viewport_tab);
    renderer.set_ui_textures(Rc::clone(&ui.ui_textures));
    renderer.render(view, &scene, &camera, &target, scene_tab);
    super::super::viewport::rebind_egui_texture(egui_renderer, &renderer.device, texture_id, view);
    let size = view.size();
    Some((size.width, size.height))
}
