//! src/shell/editor/viewport.rs — the editor's offscreen renders (the Scene/Game
//! viewport and the Inspector preview) and the Scene-tab pointer interaction.

use super::EditorFrontend;
use crate::app::GameWorld;
use crate::ecs::World;
use crate::editor::viewport::rect_tool::{self, OverlayFrame};
use crate::editor::viewport::{gizmo, pick};
use crate::editor::{EditorUi, ViewportInteraction, ViewportTab};
use crate::render::RenderView;
use crate::scene::Scene;
use crate::shell::Shell;

/// (Re)bind a freshly-rendered offscreen `view` to a stable egui texture id so the
/// `egui::Image` samples this frame's render — registering it lazily on first use.
/// Shared by the viewport, the Inspector-preview render paths (#183, #352) and the
/// headless capture. egui samples display-encoded bytes, so it gets the target's
/// display view, never the sRGB one (#333).
pub(super) fn rebind_egui_texture(
    egui_renderer: &mut egui_wgpu::Renderer,
    device: &wgpu::Device,
    id_slot: &mut Option<egui::TextureId>,
    view: &RenderView,
) {
    let Some(target_view) = view.display_view() else {
        return;
    };
    let target_view = &target_view;
    match id_slot {
        Some(id) => egui_renderer.update_egui_texture_from_wgpu_texture(
            device,
            target_view,
            wgpu::FilterMode::Linear,
            *id,
        ),
        None => {
            *id_slot = Some(egui_renderer.register_native_texture(
                device,
                target_view,
                wgpu::FilterMode::Linear,
            ));
        }
    }
}

/// Points → physical pixels for an offscreen target; `None` for a degenerate rect.
pub(super) fn target_size(size: egui::Vec2, pixels_per_point: f32) -> Option<(u32, u32)> {
    let px = (size.x * pixels_per_point).round() as u32;
    let py = (size.y * pixels_per_point).round() as u32;
    (px > 0 && py > 0).then_some((px, py))
}

impl EditorFrontend {
    /// Render the 3D scene into the offscreen viewport target sized to the panel's
    /// image rect, then (re)bind it to the stable egui texture id (#183). The active
    /// tab selects the camera + mode: **Scene** = the editor's own Scene camera with
    /// gizmos/grid (#745); **Game** = the active camera entity's view, what the player sees.
    pub(super) fn render_viewport_scene(
        &mut self,
        shell: &mut Shell,
        game: &GameWorld,
        interaction: &ViewportInteraction,
        pixels_per_point: f32,
    ) {
        let Some((px, py)) = target_size(interaction.size, pixels_per_point) else {
            return;
        };
        // The UI lays out on the game view's pixel size (#417) — a sim input.
        game.resources.screen.borrow_mut().set_game_view(px, py);
        // Create-or-resize this viewport's own view. A view's resize guard checks its
        // own size, so it never fights the preview view (#355).
        let renderer = &mut shell.renderer;
        RenderView::ensure_offscreen(
            &mut self.viewport_view,
            &renderer.device,
            renderer.config.format,
            px,
            py,
            renderer.quality.bloom_divisor(),
        );
        let Some(view) = self.viewport_view.as_mut() else {
            return;
        };
        // `wgpu::TextureView` is a refcounted handle, so this owned view doesn't
        // borrow `view` and `render` can take it as the output.
        let Some(target_view) = view.color_target_view() else {
            return;
        };

        let scene = game.scene().borrow();
        let scene_tab = interaction.tab == ViewportTab::Scene;
        view.ui.screen_in_editor = scene_tab && self.editor_ui.ui_overlay;
        let camera = viewport_camera(&self.editor_ui, game, &scene, interaction.tab);
        renderer.render(view, &scene, &camera, &target_view, scene_tab);
        rebind_egui_texture(
            &mut self.egui_renderer,
            &renderer.device,
            &mut self.viewport_texture_id,
            view,
        );
    }

    /// Render the Inspector Preview tab's isolated scene into its own offscreen target
    /// and rebind it — the viewport's two-phase contract, against
    /// `EditorUi::preview_request`/`preview_cache` instead of the game world. The
    /// rendered `Scene` is `preview_cache`'s freestanding value, never the live scene
    /// (#352). A no-op when the Preview tab isn't showing or the rect is degenerate.
    pub(super) fn render_preview_scene(&mut self, shell: &mut Shell, pixels_per_point: f32) {
        let Some(request) = self.editor_ui.preview_request.take() else {
            return;
        };
        let Some((px, py)) = target_size(request.size, pixels_per_point) else {
            return;
        };
        let renderer = &mut shell.renderer;
        RenderView::ensure_offscreen(
            &mut self.preview_view,
            &renderer.device,
            renderer.config.format,
            px,
            py,
            renderer.quality.bloom_divisor(),
        );
        let Some(view) = self.preview_view.as_mut() else {
            return;
        };
        let Some(target_view) = view.color_target_view() else {
            return;
        };
        let Some(cache) = &self.editor_ui.preview_cache else {
            return;
        };
        match &request.shader_override {
            Some(path) => renderer.render_preview_with_shader(
                view,
                &cache.scene,
                &request.camera,
                &target_view,
                path,
            ),
            None => renderer.render(view, &cache.scene, &request.camera, &target_view, false),
        }
        rebind_egui_texture(
            &mut self.egui_renderer,
            &renderer.device,
            &mut self.preview_texture_id,
            view,
        );
    }

    /// Apply the Scene-tab pointer interaction: with the UI overlay on, a drag on the
    /// selected UI element's rect-tool handles edits its RectTransform (#423);
    /// otherwise a drag on an axis handle translates the selection (same `Transform`
    /// path as the inspector). A plain click picks the UI under the cursor, then the
    /// entity (or deselects on empty space). The Game tab is view-only. Routes
    /// through the editor's `selected_entity_id`, so inspector and overlay agree.
    /// Picking works in points: the image rect is already in points.
    pub(super) fn handle_viewport_interaction(
        &mut self,
        game: &GameWorld,
        interaction: &ViewportInteraction,
        pixels_per_point: f32,
    ) {
        let ui = &mut self.editor_ui;
        let scene_tab = interaction.tab == ViewportTab::Scene;
        let Some(local) = interaction.hover_local.filter(|_| scene_tab) else {
            if !scene_tab || !interaction.dragging {
                (ui.gizmo_drag, ui.rect_drag) = (None, None);
            }
            return;
        };
        let size = interaction.size;
        let Some((ndc_x, ndc_y)) = pick::local_to_ndc(local.x, local.y, size.x, size.y) else {
            return;
        };
        let aspect = if size.y > 0.0 { size.x / size.y } else { 1.0 };
        let camera = ui.scene_view.camera.camera();
        let ray = pick::ray_from_ndc(&camera, aspect, ndc_x, ndc_y);
        let mut scene = game.scene().borrow_mut();
        let frame = OverlayFrame {
            size: glam::Vec2::new(size.x, size.y),
            pixels_per_point,
        };
        let point = glam::Vec2::new(local.x, local.y);

        // 1. Continue or start a rect-tool drag, else a gizmo drag, on the selection.
        if interaction.dragging {
            let Some(id) = ui.selected_entity_id else {
                return;
            };
            if drive_rect_tool(ui, &mut scene.world, &frame, (id, point), interaction) {
                return;
            }
            let Some(origin) = pick::entity_world_position(&scene, id) else {
                return;
            };
            if interaction.drag_started {
                // Handle pick radius scales with distance so far handles stay grabbable.
                let radius = (origin - camera.position).length() * 0.06 + 0.15;
                ui.gizmo_drag = gizmo::begin_drag(origin, ray, radius);
            }
            if let Some(drag) = ui.gizmo_drag.as_mut() {
                if gizmo::drag(&mut scene, id, drag, origin, ray) {
                    ui.is_dirty = true;
                }
            }
            return;
        }
        (ui.gizmo_drag, ui.rect_drag) = (None, None);

        // 2. A plain click selects the UI under the cursor — the overlay, then world
        // canvases in front of the nearest mesh — else that mesh, else deselects.
        if interaction.clicked {
            let hit = click_pick(&scene, &frame, point, ui.ui_overlay, ray);
            ui.selected_entity_id = hit;
            ui.selected_asset_path = None;
            scene.selected_entity_id = hit;
        }
    }
}

/// The camera a viewport tab renders from: the Scene tab's own pose, or the scene's
/// active camera entity (falling back to the game camera) on the Game tab. Shared
/// with the headless capture.
pub(super) fn viewport_camera(
    ui: &EditorUi,
    game: &GameWorld,
    scene: &Scene,
    tab: ViewportTab,
) -> crate::scene::Camera {
    match tab {
        ViewportTab::Scene => ui.scene_view.camera.camera(),
        ViewportTab::Game => crate::scene::game_camera_from_scene(&game.camera().borrow(), scene),
    }
}

/// Grab (on the drag's first frame, overlay on) or continue a rect-tool drag of
/// `(id, point)`, the selection and the pointer. Returns whether the rect tool owns
/// the drag — the move gizmo then stays out of it.
fn drive_rect_tool(
    ui: &mut EditorUi,
    world: &mut World,
    frame: &OverlayFrame,
    (id, point): (u32, glam::Vec2),
    interaction: &ViewportInteraction,
) -> bool {
    if interaction.drag_started {
        let grab = || rect_tool::begin(world, frame, id, point);
        ui.rect_drag = ui.ui_overlay.then(grab).flatten();
    }
    let Some(drag) = ui.rect_drag else {
        return false;
    };
    let d = interaction.drag_delta;
    ui.is_dirty |= rect_tool::apply(world, frame, drag, glam::Vec2::new(d.x, d.y));
    true
}

/// What a click at `point` selects: the UI under it (the overlay while shown, then
/// world canvases in front of the nearest mesh), else that mesh.
fn click_pick(
    scene: &Scene,
    frame: &OverlayFrame,
    point: glam::Vec2,
    overlay: bool,
    ray: pick::Ray,
) -> Option<u32> {
    let mesh = pick::pick_entity(scene, ray);
    let limit = mesh.map_or(f32::INFINITY, |(_, toi)| toi);
    let ray = (ray.origin, ray.dir);
    let widget = rect_tool::pick(&scene.world, frame, point, overlay, ray, limit);
    widget.or(mesh.map(|(id, _)| id))
}
