//! src/render/ui/draw.rs — the UI pass: per-view vertex buffers + the draw (#418).
//!
//! [`Renderer::prepare_ui`] runs inside `Renderer::render` before the camera stack:
//! the layout is recomputed for the view's own pixel size through its camera (the
//! same pure `UiLayout::compute_in` the sim's `LateUpdate` system runs, so in Play
//! it equals `Resources::ui_layout` whenever the view is the game view) and turned
//! into per-canvas meshes. World canvases then draw inside the stack (`world`,
//! #429); [`Renderer::draw_ui`] draws the screen canvases right after post-FX, with
//! `LoadOp::Load` over the finished frame. The Scene view (editor mode) keeps only
//! `WorldSpace` canvases — signs and terminals are scene geometry; the game's HUD
//! and a camera canvas glued to the editor's free-fly camera are not.
//!
//! **Dirty strategy.** Each view keeps one vertex buffer per canvas (`cache`);
//! rebuilding the CPU mesh is a cheap walk, and a canvas's buffer is re-uploaded
//! only when its mesh changed. A static HUD therefore costs no uploads per frame.
//!
//! **Effects.** After the sync, every batch's clip slot is written and each Mask's
//! coverage texture rendered (`effects`, #428), before the camera stack — world
//! canvases need them too. Right before the overlay pass, the backdrop blur runs
//! over the finished frame (`effects::blur`, #426), only when a backdrop shows.

use std::rc::Rc;

use glam::Vec2;

use super::cache::{CanvasPlace, UiViewCache};
use super::effects::{color_pass, GroupKey};
use super::mesh::build_canvas_meshes;
use crate::render::{RenderView, Renderer};
use crate::scene::{Camera, Scene};
use crate::ui::{CanvasSpace, UiLayout, UiView};

impl Renderer {
    /// Lay out and mesh `scene`'s UI for `view` through `camera`, uploading only
    /// what changed (see the module docs). Runs before the camera stack.
    pub(crate) fn prepare_ui(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        camera: &Camera,
        editor_mode: bool,
    ) {
        (view.ui.uploads, view.ui.drawn) = (0, 0);
        let size = view.size();
        let screen = Vec2::new(size.width as f32, size.height as f32);
        let ui_view = UiView::with_camera(screen, camera.clone());
        let layout = UiLayout::compute_in(&scene.world, &ui_view);
        for path in texture_paths(scene, &layout) {
            let tex = self.load_texture(&path);
            self.ui_renderer
                .bind_group(&self.device, Some((&path, Rc::clone(&tex))));
        }
        let textures = &self.gpu_textures;
        let tex_size = |p: &str| {
            let s = textures.get(p)?.texture.size();
            Some(Vec2::new(s.width as f32, s.height as f32))
        };
        let atlases = &mut self.ui_renderer.atlases;
        let meshes = build_canvas_meshes(&scene.world, &layout, screen, &tex_size, atlases);
        self.ui_renderer.upload_atlases(&self.device, &self.queue);
        let world = &scene.world;
        let overlay = view.ui.screen_in_editor;
        let meshes = meshes
            .into_iter()
            .filter(|m| {
                !editor_mode
                    || overlay
                    || world.canvas(m.canvas).is_some_and(|c| c.is_world_space())
            })
            .map(|m| {
                let place = CanvasPlace {
                    space: layout.space(m.canvas),
                    size: layout.get(m.canvas).map_or(Vec2::ONE, |r| r.rect.1),
                    layer: world.layer(m.canvas),
                };
                (m, place)
            })
            .collect();
        view.ui.sync(&self.device, &self.queue, meshes);
        self.upload_batch_uniforms(&mut view.ui);
        self.render_masks(view);
    }

    /// Draw the screen canvases [`Renderer::prepare_ui`] synced over `view`'s
    /// finished frame. A no-op for a view with nowhere to draw them (a targetless
    /// view given no `set_ui_output`: the cubemap capture).
    pub(crate) fn draw_ui(&mut self, view: &mut RenderView) {
        let Some((target, format)) = view.take_ui_target() else {
            return;
        };
        let screen = view.ui.places().filter(|p| p.space == CanvasSpace::Screen);
        let drawn = screen.count();
        if drawn == 0 {
            return;
        }
        view.ui.drawn += drawn;
        self.ui_renderer.ensure_pipeline(&self.device, format);
        self.run_backdrop_blur(view);
        self.encode_ui(&view.ui, &target, format, view.size());
    }

    /// Record and submit the UI pass over `target`.
    fn encode_ui(
        &self,
        cache: &UiViewCache,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        size: winit::dpi::PhysicalSize<u32>,
    ) {
        let ui = &self.ui_renderer;
        let screen = || {
            let all = cache.canvases().iter().enumerate();
            all.filter(|(_, c)| c.place.space == CanvasSpace::Screen)
        };
        let divisor = Some(self.quality.backdrop_divisor());
        let key = |b| GroupKey::of(b, divisor);
        let keys = screen().flat_map(|(_, c)| c.mesh.batches.iter().map(key));
        let groups = self.batch_groups(&cache.effects, keys);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("UI Encoder"),
            });
        {
            let mut pass = color_pass(&mut encoder, "UI Pass", target, wgpu::LoadOp::Load);
            let mut bound = None;
            for (i, canvas) in screen() {
                pass.set_vertex_buffer(0, canvas.buffer.slice(..));
                for (b, batch) in canvas.mesh.batches.iter().enumerate() {
                    // A backdrop always composites "over" (#426); else the blend mode.
                    let mode = (batch.blend, batch.backdrop.is_some());
                    if bound != Some(mode) {
                        let pipeline = match mode.1 {
                            true => ui.backdrops.get(&format),
                            false => ui.pipelines.get(&(format, batch.blend)),
                        };
                        let Some(pipeline) = pipeline else { continue };
                        pass.set_pipeline(pipeline);
                        bound = Some(mode);
                    }
                    pass.set_bind_group(0, ui.source_group(&batch.source), &[]);
                    let slot = cache.effects.batch_offset(i, b);
                    pass.set_bind_group(1, &groups[&key(batch)], &[slot]);
                    let s = batch
                        .clip
                        .rect
                        .map_or((0, 0, size.width, size.height), |s| (s.x, s.y, s.w, s.h));
                    pass.set_scissor_rect(s.0, s.1, s.2, s.3);
                    pass.draw(batch.range.clone(), 0..1);
                }
            }
        }
        self.queue.submit(Some(encoder.finish()));
    }
}

/// Every texture path an active, laid-out Image samples — gathered first so the
/// scene borrow ends before the textures load.
fn texture_paths(scene: &Scene, layout: &UiLayout) -> Vec<String> {
    let mut paths: Vec<String> = layout
        .iter()
        .filter_map(|(id, _)| Some(scene.world.image(id)?.shown_texture()?.to_string()))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod draw_tests;
