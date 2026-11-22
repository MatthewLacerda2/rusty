use crate::gpu::commands::Commands;
use crate::ui::pipeline::{BATCH_GROUP, SOURCE_GROUP};
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
    pub(crate) fn prepare_ui(
        &mut self,
        commands: &mut Commands,
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
        self.bind_ui_textures(scene, &layout);
        self.lap("ui.textures");
        let textures = &self.gpu_textures;
        let tex_size = |p: &str| {
            // Source pixels, as the sim lays out with: an import's Max Size may shrink the upload.
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
        self.ui_renderer
            .upload_atlases(&self.device, (&self.queue, &self.staging));
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
        self.render_masks(commands.encoder(), view);
    }

    pub(crate) fn draw_ui(&mut self, commands: &mut Commands, view: &mut RenderView) {
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
        self.run_backdrop_blur(commands.encoder(), view);
        self.lap("ui.blur");
        self.screen_groups(&mut view.ui);
        self.lap("ui.groups");
        let pass = (&view.ui, &target, format);
        self.encode_ui(commands.encoder(), pass, screen_box(view));
    }

    fn screen_groups(&self, cache: &mut UiViewCache) {
        let batches = screen_canvases(cache).flat_map(|(_, c)| c.mesh.batches.iter());
        let keys: Vec<_> = batches.map(|b| self.screen_key(b)).collect();
        self.batch_groups(&mut cache.effects, keys);
    }

    fn screen_key(&self, batch: &UiBatch) -> GroupKey {
        GroupKey::of(batch, Some(self.quality.backdrop_divisor()))
    }

    fn encode_ui(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        (cache, target, format): (&UiViewCache, &wgpu::TextureView, wgpu::TextureFormat),
        (x, y, width, height): (u32, u32, u32, u32),
    ) {
        let ui = &self.ui_renderer;
        {
            let mut pass = color_pass(
                encoder,
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
                    pass.set_bind_group(SOURCE_GROUP, ui.source_group(&batch.source), &[]);
                    let slot = cache.effects.batch_offset(i, b);
                    pass.set_bind_group(
                        BATCH_GROUP,
                        cache.effects.group(&self.screen_key(batch)),
                        &[slot],
                    );
                    let s = batch
                        .clip
                        .rect
                        .map_or((0, 0, width, height), |s| (s.x, s.y, s.w, s.h));
                    pass.set_scissor_rect(x + s.0, y + s.1, s.2, s.3);
                    pass.draw(batch.range.clone(), 0..1);
                }
            }
        }
        self.lap("ui.encode");
    }
}

fn screen_canvases(cache: &UiViewCache) -> impl Iterator<Item = (usize, &CanvasGpu)> {
    let all = cache.canvases().iter().enumerate();
    all.filter(|(_, c)| c.place.space == CanvasSpace::Screen)
}

fn screen_box(view: &RenderView) -> (u32, u32, u32, u32) {
    let size = view.size();
    let Some((x, y, w, h)) = view.ui.screen_box else {
        return (0, 0, size.width, size.height);
    };
    let (sw, sh) = (size.width.max(1), size.height.max(1));
    let (x, y) = (x.min(sw - 1), y.min(sh - 1));
    (x, y, w.clamp(1, sw - x), h.clamp(1, sh - y))
}

impl Renderer {
    fn bind_ui_textures(&mut self, scene: &Scene, layout: &UiLayout) {
        let due = self.notes_due();
        for path in texture_paths(scene, layout) {
            let tex = self.load_texture(&path);
            let rebuilt =
                (self.ui_renderer).bind_group(&self.device, path.as_str(), Rc::clone(&tex));
            if due || rebuilt {
                self.note_map(scene.id(), path.as_str());
            }
        }
    }
}

impl super::UiRenderer {
    fn bind_group(
        &mut self,
        device: &wgpu::Device,
        path: &str,
        tex: Rc<crate::GpuTexture>,
    ) -> bool {
        let fresh = self
            .textures
            .get(path)
            .is_some_and(|(cached, _)| Rc::ptr_eq(cached, &tex));
        if !fresh {
            let sampler = self
                .image_samplers
                .entry(tex.filter)
                .or_insert_with(|| crate::gpu::textures::ui_sampler(device, tex.filter));
            let group = super::bind(device, &self.layout, &tex.view, sampler);
            self.textures.insert(path.to_string(), (tex, group));
        }
        !fresh
    }

    pub(crate) fn forget_textures(&mut self, gone: impl Fn(&str) -> bool) {
        self.textures.retain(|path, _| !gone(path));
    }
}

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
