use super::EditorFrontend;
use crate::Shell;
use rusty_app::GameWorld;
use rusty_core::application::Gizmo;
use rusty_ecs::World;
use rusty_editor::viewport::rect_tool::{self, OverlayFrame};
use rusty_editor::viewport::{game_size, gizmo, pick};
use rusty_editor::{EditorUi, ViewportInteraction, ViewportTab};
use rusty_render::{RenderView, Renderer};
use rusty_scene::Scene;

/// One stable egui id per slot, so the `egui::Image` samples this frame's render.
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

/// Render the Inspector Preview tab's isolated scene into its own offscreen `view`, then
/// rebind it: the window and the capture both call this.
pub(super) fn render_preview_scene(
    (renderer, egui_renderer): (&mut Renderer, &mut egui_wgpu::Renderer),
    ui: &mut EditorUi,
    (view, texture_id): (&mut Option<RenderView>, &mut Option<egui::TextureId>),
    pixels_per_point: f32,
) {
    let Some(request) = ui.preview_request.take() else {
        return;
    };
    let Some((px, py)) = target_size(request.rect.size(), pixels_per_point) else {
        return;
    };
    // A preview is not the screen: it renders natively.
    let format = renderer.config.format;
    let view =
        RenderView::ensure_offscreen(view, &renderer.device, format, (px, py), Default::default());
    let (Some(target_view), Some(cache)) = (view.color_target_view(), &ui.preview_cache) else {
        return;
    };
    renderer.aside(|renderer| match &request.shader_override {
        Some(path) => renderer.render_preview_with_shader(
            view,
            &cache.scene,
            &request.camera,
            &target_view,
            path,
        ),
        None => renderer.render(view, &cache.scene, &request.camera, &target_view, false),
    });
    rebind_egui_texture(egui_renderer, &renderer.device, texture_id, view);
}

impl EditorFrontend {
    pub(super) fn render_viewport_scene(
        &mut self,
        shell: &mut Shell,
        game: &GameWorld,
        interaction: &ViewportInteraction,
        pixels_per_point: f32,
    ) {
        let Some(panel) = target_size(interaction.size, pixels_per_point) else {
            return;
        };
        let scene_tab = interaction.tab == ViewportTab::Scene;
        let (px, py) = interaction.render.unwrap_or(panel);
        if self.demo_size().is_none() {
            let (sx, sy) = interaction.screen.unwrap_or(panel);
            game.resources.screen.borrow_mut().set_game_view(sx, sy);
        }
        let renderer = &mut shell.renderer;
        let view = RenderView::ensure_offscreen(
            &mut self.viewport_views[usize::from(!scene_tab)],
            &renderer.device,
            renderer.config.format,
            (px, py),
            renderer.render_scale(scene_tab),
        );
        let Some(target_view) = view.color_target_view() else {
            return;
        };

        let scene = game.scene().borrow();
        view.gizmos = self.editor_ui.gizmos;
        if scene_tab {
            super::navmesh::hand(renderer, &view.gizmos, &game.resources.nav.borrow());
        }
        view.game_gizmos = !scene_tab && view.gizmos.on(Gizmo::GameGizmos);
        view.ui.screen_in_editor = scene_tab && view.gizmos.on(Gizmo::Ui);
        view.ui.screen_box = game_size::overlay_box(interaction, (px, py));
        view.set_pixel_scale(pixels_per_point);
        let camera = viewport_camera(&self.editor_ui, game, &scene, interaction.tab);
        renderer.set_ui_textures(std::rc::Rc::clone(
            game.resources.screen.borrow().textures(),
        ));
        renderer.render(view, &scene, &camera, &target_view, scene_tab);
        drop(scene);
        #[cfg(feature = "dev")]
        if !scene_tab {
            game.scene().borrow_mut().screen_exposure = view.exposure_readout();
        }
        rebind_egui_texture(
            &mut self.egui_renderer,
            &renderer.device,
            &mut self.viewport_texture_id,
            view,
        );
    }

    pub(super) fn render_preview(&mut self, shell: &mut Shell, pixels_per_point: f32) {
        let gpu = (&mut shell.renderer, &mut self.egui_renderer);
        let target = (&mut self.preview_view, &mut self.preview_texture_id);
        render_preview_scene(gpu, &mut self.editor_ui, target, pixels_per_point);
    }

    /// The size of the Game view an open demo replays on; `None` with none open.
    pub(super) fn demo_size(&self) -> Option<(u32, u32)> {
        #[cfg(feature = "dev")]
        return self.editor_ui.demo_size();
        // lint: allow(gpu-default-only) ship stub: no demo exists without dev, and `None` is what a dev build reads with none open
        #[cfg(not(feature = "dev"))]
        None
    }

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
        let frame = game_size::overlay_frame(interaction, pixels_per_point, &ui.ui_textures);
        let point = glam::Vec2::new(local.x, local.y);

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
                // World units, grown with distance: far handles stay grabbable.
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

        if interaction.clicked {
            let hit = click_pick(&scene, &frame, point, ui.gizmos.on(Gizmo::Ui), ray);
            ui.selected_entity_id = hit;
            ui.selected_asset_path = None;
            scene.selected_entity_id = hit;
        }
    }
}

pub(super) fn viewport_camera(
    ui: &EditorUi,
    game: &GameWorld,
    scene: &Scene,
    tab: ViewportTab,
) -> rusty_scene::Camera {
    match tab {
        ViewportTab::Scene => ui.scene_view.camera.camera(),
        ViewportTab::Game => rusty_scene::game_camera_from_scene(&game.camera().borrow(), scene),
    }
}

/// Whether the rect tool owns the drag; the move gizmo then stays out of it.
fn drive_rect_tool(
    ui: &mut EditorUi,
    world: &mut World,
    frame: &OverlayFrame,
    (id, point): (u32, glam::Vec2),
    interaction: &ViewportInteraction,
) -> bool {
    if interaction.drag_started {
        let grab = || rect_tool::begin(world, frame, id, point);
        ui.rect_drag = ui.gizmos.on(Gizmo::Ui).then(grab).flatten();
    }
    let Some(drag) = ui.rect_drag else {
        return false;
    };
    let d = interaction.drag_delta;
    ui.is_dirty |= rect_tool::apply(world, frame, drag, glam::Vec2::new(d.x, d.y));
    true
}

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
