//! crates/render/src/ui/draw.rs — the UI pass: per-view vertex buffers + the draw (#418).
//!
//! [`Renderer::prepare_ui`] runs inside `Renderer::render` before the camera stack:
//! the view lays its UI out at its own pixel size through its camera (the
//! same pure `UiLayout::compute_in` the sim's `LateUpdate` system runs, with the
//! sim's own texture-size table ([`Renderer::set_ui_textures`]) — so in Play it
//! equals `Resources::ui_layout` whenever the view is the game view) and turned
//! into per-canvas meshes, each kept while its inputs hold. World canvases then draw inside the stack (`world`,
//! #429); [`Renderer::draw_ui`] draws the screen canvases right after post-FX, with
//! `LoadOp::Load` over the finished frame. The Scene view (editor mode) keeps only
//! `WorldSpace` canvases — signs and terminals are scene geometry; the game's HUD
//! and a camera canvas glued to the editor's free-fly camera are not.
//!
//! **Dirty strategy.** Each view keeps its layout with what it was computed from
//! (`kept::layout`): a frame where nothing the layout reads changed lays out
//! nothing, and places the held rects' canvas spaces again where they can move (a
//! world or camera canvas). It keeps every root canvas's mesh
//! with what it was built from too (`kept`): a canvas whose rects,
//! frame, texture sizes and glyph atlases are unchanged and whose UI components no
//! write touched (the World's change log) keeps its mesh unbuilt. One vertex buffer
//! per canvas (`cache`) is re-uploaded only when a rebuilt mesh differs. A still HUD
//! therefore meshes nothing and uploads nothing per frame; a canvas that moves (a
//! turning minimap) is rebuilt alone.
//!
//! **Effects.** After the sync, every batch's clip slot is written and each Mask's
//! coverage texture rendered (`effects`, #428), before the camera stack — world
//! canvases need them too. Right before the overlay pass, the backdrop blur runs
//! over the finished frame (`effects::blur`, #426), only when a backdrop shows.
//!
//! **Laps.** Both halves cut their CPU time into `ui.<part>` laps (`Renderer::lap`),
//! which the bench reports as `render.ui.<part>` inside `render.ui`: `layout`,
//! `textures` (loading and binding the Images' textures), `mesh` (the canvases'
//! inputs compared, the changed ones meshed, and the glyph atlases), `sync`, `shades`, `uniforms`, `masks` and `mask_submit`, then
//! `pipelines` (making the overlay's pipelines current), `blur` (the backdrop
//! chain alone), `groups`, `encode` and `submit` for the overlay pass.

use crate::gpu::ops::CountedQueue as _;
use std::rc::Rc;

use glam::Vec2;

use super::cache::{CanvasGpu, CanvasPlace, UiViewCache};
use super::effects::GroupKey;
use super::mesh::UiBatch;
use super::pipeline::UiPass;
use crate::gpu::fullscreen::color_pass;
use crate::timing::GpuPass;
use crate::{RenderView, Renderer};
use rusty_asset::MapRef;
use rusty_scene::{Camera, Scene};
use rusty_ui::{CanvasSpace, UiLayout, UiView};

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
        let (_, _, w, h) = screen_box(view);
        let screen = Vec2::new(w as f32, h as f32);
        let textures = Rc::clone(&self.ui_renderer.texture_sizes);
        let ui_view = UiView::with_camera(screen, camera.clone(), textures);
        let layout = view.ui.layout.update(&scene.world, &ui_view);
        self.lap("ui.layout");
        for path in texture_paths(scene, &layout) {
            let tex = self.load_texture(&path);
            self.ui_renderer
                .bind_group(&self.device, Some((path.as_str(), Rc::clone(&tex))));
        }
        self.lap("ui.textures");
        let textures = &self.gpu_textures;
        // Source pixels, not the upload's, which *Max Size* may have shrunk (#1574).
        let tex_size = |p: &str| {
            let (w, h) = textures.get(p)?.source_size;
            Some(Vec2::new(w as f32, h as f32))
        };
        let world = &scene.world;
        let overlay = view.ui.screen_in_editor;
        let wanted =
            |c: u32| !editor_mode || overlay || world.canvas(c).is_some_and(|c| c.is_world_space());
        let atlases = &mut self.ui_renderer.atlases;
        let looked = (&tex_size as &dyn Fn(&str) -> Option<Vec2>, atlases);
        let meshes = view
            .ui
            .kept
            .update((world, &*layout), screen, looked, &wanted);
        self.ui_renderer.upload_atlases(&self.device, &self.queue);
        self.lap("ui.mesh");
        let meshes = meshes
            .into_iter()
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
        self.lap("ui.sync");
        self.prepare_ui_shades(view, scene.ui_time);
        self.lap("ui.shades");
        self.upload_batch_uniforms(&mut view.ui);
        self.lap("ui.uniforms");
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
        self.ui_renderer.shaders.ensure_screen(&self.device, format);
        self.lap("ui.pipelines");
        self.run_backdrop_blur(view);
        self.lap("ui.blur");
        self.screen_groups(&mut view.ui);
        self.lap("ui.groups");
        self.encode_ui(&view.ui, &target, format, screen_box(view));
    }

    /// Make the batch group of every screen canvas's batch current.
    fn screen_groups(&self, cache: &mut UiViewCache) {
        let batches = screen_canvases(cache).flat_map(|(_, c)| c.mesh.batches.iter());
        let keys: Vec<_> = batches.map(|b| self.screen_key(b)).collect();
        self.batch_groups(&mut cache.effects, keys);
    }

    /// The batch group a screen batch binds: the one key [`Self::screen_groups`]
    /// makes and [`Self::encode_ui`] reads.
    fn screen_key(&self, batch: &UiBatch) -> GroupKey {
        GroupKey::of(batch, Some(self.quality.backdrop_divisor()))
    }

    /// Record and submit the UI pass over `target`.
    fn encode_ui(
        &self,
        cache: &UiViewCache,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        (x, y, width, height): (u32, u32, u32, u32),
    ) {
        let ui = &self.ui_renderer;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("UI Encoder"),
            });
        {
            let mut pass = color_pass(
                &mut encoder,
                ("UI Pass", &self.gpu_timer, GpuPass::Ui),
                target,
                wgpu::LoadOp::Load,
            );
            pass.set_viewport(x as f32, y as f32, width as f32, height as f32, 0.0, 1.0);
            let mut bound = None;
            for (i, canvas) in screen_canvases(cache) {
                pass.set_vertex_buffer(0, canvas.buffer.slice(..));
                for (b, batch) in canvas.mesh.batches.iter().enumerate() {
                    let at = (cache, i, b);
                    if !ui.bind_batch(&mut pass, at, UiPass::Screen(format), &mut bound) {
                        continue;
                    }
                    pass.set_bind_group(0, ui.source_group(&batch.source), &[]);
                    let slot = cache.effects.batch_offset(i, b);
                    pass.set_bind_group(1, cache.effects.group(&self.screen_key(batch)), &[slot]);
                    let s = batch
                        .clip
                        .rect
                        .map_or((0, 0, width, height), |s| (s.x, s.y, s.w, s.h));
                    pass.set_scissor_rect(x + s.0, y + s.1, s.2, s.3);
                    pass.draw(batch.range.clone(), 0..1);
                }
            }
        }
        let commands = encoder.finish();
        self.lap("ui.encode");
        self.queue.counted_submit(Some(commands));
        self.lap("ui.submit");
    }
}

/// The screen canvases (the overlay pass's), by index in draw order: the one
/// filter the groups and the pass share.
fn screen_canvases(cache: &UiViewCache) -> impl Iterator<Item = (usize, &CanvasGpu)> {
    let all = cache.canvases().iter().enumerate();
    all.filter(|(_, c)| c.place.space == CanvasSpace::Screen)
}

/// The rect the screen canvases lay out and draw in: the view's `screen_box`,
/// kept inside the view, else the whole view.
fn screen_box(view: &RenderView) -> (u32, u32, u32, u32) {
    let size = view.size();
    let Some((x, y, w, h)) = view.ui.screen_box else {
        return (0, 0, size.width, size.height);
    };
    let (sw, sh) = (size.width.max(1), size.height.max(1));
    let (x, y) = (x.min(sw - 1), y.min(sh - 1));
    (x, y, w.clamp(1, sw - x), h.clamp(1, sh - y))
}

/// Every texture path an active, laid-out Image samples — gathered first so the
/// scene borrow ends before the textures load.
fn texture_paths(scene: &Scene, layout: &UiLayout) -> Vec<MapRef> {
    let mut paths: Vec<MapRef> = layout
        .iter()
        .filter_map(|(id, _)| scene.world.image(id)?.shown_texture().cloned())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod draw_tests;
