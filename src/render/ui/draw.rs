//! src/render/ui/draw.rs — the UI pass: per-view vertex buffers + the draw (#418).
//!
//! Runs inside `Renderer::render` right after post-FX. The layout is recomputed for
//! the view's own pixel size (the same pure `UiLayout::compute` the sim's
//! `LateUpdate` system runs, so in Play it equals `Resources::ui_layout` whenever
//! the view is the game view), turned into per-canvas meshes, and drawn with
//! `LoadOp::Load` over the finished frame.
//!
//! **Dirty strategy.** Each view keeps one vertex buffer per canvas plus the mesh it
//! holds. Rebuilding the CPU mesh is a cheap walk; a canvas's buffer is re-uploaded
//! only when its mesh differs from last frame's (a layout or graphic change), and
//! reallocated only when it outgrows its capacity. A static HUD therefore costs no
//! uploads per frame.

use std::collections::HashMap;
use std::rc::Rc;

use glam::Vec2;

use super::mesh::{build_canvas_meshes, CanvasMesh, UiVertex};
use crate::render::{RenderView, Renderer};
use crate::scene::Scene;
use crate::ui::UiLayout;

/// The vertex buffer layout matching `ui.wgsl`'s `VertexIn`.
pub(crate) fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x2, // pos (NDC)
        1 => Float32x2, // uv
        2 => Float32x4, // color
    ];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBS,
    }
}

/// One canvas's GPU copy of its mesh.
struct CanvasGpu {
    mesh: CanvasMesh,
    buffer: wgpu::Buffer,
}

/// A view's UI vertex buffers, one per drawn canvas in draw order (see the module
/// docs for when they are re-uploaded).
#[derive(Default)]
pub struct UiViewCache {
    canvases: Vec<CanvasGpu>,
    /// How many canvas buffers the last frame (re-)uploaded — the dirty signal.
    uploads: usize,
    /// How many canvases the last frame drew.
    drawn: usize,
}

impl UiViewCache {
    /// UI batches (draw calls) the last render of this view issued.
    pub fn last_batches(&self) -> usize {
        match self.drawn {
            0 => 0,
            _ => self.canvases.iter().map(|c| c.mesh.batches.len()).sum(),
        }
    }

    /// Canvas buffers (re-)uploaded by the last render of this view.
    pub fn last_uploads(&self) -> usize {
        self.uploads
    }

    /// Canvases the last render of this view drew (0 when it had nowhere to draw).
    pub fn last_drawn(&self) -> usize {
        self.drawn
    }

    /// Adopt this frame's `meshes`, uploading only the ones that changed.
    fn sync(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, meshes: Vec<CanvasMesh>) {
        let mut old: HashMap<u32, CanvasGpu> = self
            .canvases
            .drain(..)
            .map(|c| (c.mesh.canvas, c))
            .collect();
        self.uploads = 0;
        for mesh in meshes {
            let bytes: &[u8] = bytemuck::cast_slice(&mesh.vertices);
            let gpu = match old.remove(&mesh.canvas) {
                Some(c) if c.mesh == mesh => c,
                Some(c) if c.buffer.size() >= bytes.len() as u64 => {
                    queue.write_buffer(&c.buffer, 0, bytes);
                    self.uploads += 1;
                    CanvasGpu { mesh, ..c }
                }
                _ => {
                    self.uploads += 1;
                    let buffer = create_buffer(device, bytes);
                    CanvasGpu { mesh, buffer }
                }
            };
            self.canvases.push(gpu);
        }
    }
}

/// A vertex buffer holding `bytes`, with room to grow (the next power of two).
fn create_buffer(device: &wgpu::Device, bytes: &[u8]) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    let mut contents = bytes.to_vec();
    contents.resize(bytes.len().next_power_of_two().max(256), 0);
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("UI Canvas Vertices"),
        contents: &contents,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    })
}

impl Renderer {
    /// Draw every visible canvas of `scene` over `view`'s finished frame. A no-op
    /// for a scene without UI and for a view with nowhere to draw it (a targetless
    /// view given no `set_ui_output`: the cubemap capture).
    pub(crate) fn draw_ui(&mut self, view: &mut RenderView, scene: &Scene) {
        (view.ui.uploads, view.ui.drawn) = (0, 0);
        let Some((target, format)) = view.take_ui_target() else {
            return;
        };
        let size = view.size();
        let screen = Vec2::new(size.width as f32, size.height as f32);
        let layout = UiLayout::compute(&scene.world, screen);
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
        let meshes = build_canvas_meshes(&scene.world, &layout, screen, &tex_size);
        view.ui.sync(&self.device, &self.queue, meshes);
        view.ui.drawn = view.ui.canvases.len();
        if view.ui.drawn == 0 {
            return;
        }
        self.ui_renderer.ensure_pipeline(&self.device, format);
        self.encode_ui(&view.ui, &target, format, size);
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
        let Some(pipeline) = ui.pipelines.get(&format) else {
            return;
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("UI Encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(pipeline);
            for canvas in &cache.canvases {
                pass.set_vertex_buffer(0, canvas.buffer.slice(..));
                for batch in &canvas.mesh.batches {
                    let group = batch
                        .texture
                        .as_ref()
                        .and_then(|p| ui.textures.get(p))
                        .map_or(&ui.white, |(_, g)| g);
                    pass.set_bind_group(0, group, &[]);
                    let s = batch
                        .clip
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
        .filter_map(|(id, _)| scene.world.image(id)?.texture.clone())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod draw_tests;
