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
//! **Dirty strategy.** Each view keeps one vertex buffer per canvas plus the mesh it
//! holds. Rebuilding the CPU mesh is a cheap walk; a canvas's buffer is re-uploaded
//! only when its mesh differs from last frame's (a layout or graphic change), and
//! reallocated only when it outgrows its capacity. A static HUD therefore costs no
//! uploads per frame.

use std::collections::HashMap;
use std::rc::Rc;

use glam::Vec2;

use super::mesh::{build_canvas_meshes, CanvasMesh, UiVertex};
use super::world::{CanvasPlace, WorldUniforms};
use crate::render::{RenderView, Renderer};
use crate::scene::{Camera, Scene};
use crate::ui::{CanvasSpace, UiLayout, UiView};

/// The vertex buffer layout matching `ui.wgsl`'s `VertexIn`.
pub(crate) fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        0 => Float32x2, // pos (NDC)
        1 => Float32x2, // uv
        2 => Float32x4, // color
        3 => Float32x4, // outline colour (text)
        4 => Float32x4, // glow colour (text)
        5 => Float32x4, // sdf: [is text, dilate, outline, glow]
    ];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBS,
    }
}

/// One canvas's GPU copy of its mesh, and where it draws this frame.
pub(super) struct CanvasGpu {
    pub(super) mesh: CanvasMesh,
    pub(super) buffer: wgpu::Buffer,
    place: CanvasPlace,
}

/// A view's UI vertex buffers, one per drawn canvas in draw order (see the module
/// docs for when they are re-uploaded).
#[derive(Default)]
pub struct UiViewCache {
    canvases: Vec<CanvasGpu>,
    /// How many canvas buffers the last frame (re-)uploaded — the dirty signal.
    uploads: usize,
    /// How many canvases the last frame drew (a world canvas once per camera).
    pub(super) drawn: usize,
    /// The world canvases' per-camera uniforms (#429), made on first use.
    pub(super) world: Option<WorldUniforms>,
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

    /// Synced canvas `i`, in draw order.
    pub(super) fn canvas(&self, i: usize) -> &CanvasGpu {
        &self.canvases[i]
    }

    /// Every synced canvas's placement, in draw order.
    pub(super) fn places(&self) -> impl Iterator<Item = CanvasPlace> + '_ {
        self.canvases.iter().map(|c| c.place)
    }

    /// Adopt this frame's `meshes`, uploading only the ones that changed.
    fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        meshes: Vec<(CanvasMesh, CanvasPlace)>,
    ) {
        let mut old: HashMap<u32, CanvasGpu> = self
            .canvases
            .drain(..)
            .map(|c| (c.mesh.canvas, c))
            .collect();
        self.uploads = 0;
        for (mesh, place) in meshes {
            let bytes: &[u8] = bytemuck::cast_slice(&mesh.vertices);
            let gpu = match old.remove(&mesh.canvas) {
                Some(c) if c.mesh == mesh => CanvasGpu { place, ..c },
                Some(c) if c.buffer.size() >= bytes.len() as u64 => {
                    queue.write_buffer(&c.buffer, 0, bytes);
                    self.uploads += 1;
                    CanvasGpu { mesh, place, ..c }
                }
                _ => {
                    self.uploads += 1;
                    let buffer = create_buffer(device, bytes);
                    CanvasGpu {
                        mesh,
                        buffer,
                        place,
                    }
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
        let meshes = meshes
            .into_iter()
            .filter(|m| !editor_mode || world.canvas(m.canvas).is_some_and(|c| c.is_world_space()))
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
            let screen = cache
                .canvases
                .iter()
                .filter(|c| c.place.space == CanvasSpace::Screen);
            for canvas in screen {
                pass.set_vertex_buffer(0, canvas.buffer.slice(..));
                for batch in &canvas.mesh.batches {
                    pass.set_bind_group(0, ui.source_group(&batch.source), &[]);
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
        .filter_map(|(id, _)| Some(scene.world.image(id)?.shown_texture()?.to_string()))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod draw_tests;
