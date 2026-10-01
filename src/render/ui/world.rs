//! src/render/ui/world.rs — world canvases drawn in the scene (#429).
//!
//! `WorldSpace` and `ScreenSpaceCamera` canvases are geometry in the world: after
//! each stacked camera's transparent pass (and before its particles and the post-FX
//! chain) their cached meshes are drawn into the HDR scene target, depth-tested
//! against the camera's depth without writing it — so a wall in front occludes a
//! sign, and the canvas is fogged, tonemapped and bloomed like the world around it.
//! A canvas draws in a camera whose culling mask includes the canvas entity's
//! layer; canvases draw back to front by distance, each in hierarchy order.
//!
//! The vertices are the same NDC the overlay uses (a world canvas's "screen" is its
//! own rect, `mesh`); one uniform per canvas maps them onto its plane
//! ([`CanvasSpace::World`]) and through the camera. They are rewritten per camera,
//! never the vertex buffers, so a static sign uploads nothing while the view moves.
//! A scissor cannot follow a plane in perspective, so `RectMask` clips in canvas
//! space instead (#619): each batch gets its own uniform slot carrying its clip as
//! NDC bounds, and the fragment shader drops what falls outside — the same space
//! the hit-test checks masks in, so what draws is what is hit.

use glam::{Mat4, Vec2};

use super::draw::UiViewCache;
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::gpu::uniforms::FogUniform;
use crate::render::postfx::HDR_FORMAT;
use crate::render::{RenderView, Renderer};
use crate::scene::Camera;
use crate::ui::CanvasSpace;

/// One world canvas's placement for one camera — matches `ui.wgsl`'s `WorldUi`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct WorldUiUniform {
    view_proj: [f32; 16],
    to_world: [f32; 16],
    eye: [f32; 4],
    /// The batch's clip, canvas NDC `[min.x, min.y, max.x, max.y]`.
    clip: [f32; 4],
    fog: FogUniform,
}

/// Bounds that clip nothing (a graphic may overflow its canvas's rect).
const NO_CLIP: [f32; 4] = [-f32::MAX, -f32::MAX, f32::MAX, f32::MAX];

/// The world pass's shared GPU state: its uniform layout and pipeline.
pub(crate) struct WorldUiPipeline {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

impl WorldUiPipeline {
    /// Build the pipeline: premultiplied alpha into the HDR target, depth-tested
    /// (`LessEqual`) but never written, no culling (a canvas reads from both sides).
    pub(crate) fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        texture_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("World UI Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<WorldUiUniform>() as u64
                    ),
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("World UI Pipeline Layout"),
            bind_group_layouts: &[texture_layout, &layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("World UI Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vs_world",
                buffers: &[super::draw::vertex_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: "fs_world",
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        Self { layout, pipeline }
    }
}

/// A view's world-canvas uniforms: one aligned slot per canvas batch, rewritten
/// per camera.
pub(crate) struct WorldUniforms {
    buffer: GrowBuffer,
    group: wgpu::BindGroup,
}

impl Renderer {
    /// Draw `view`'s world canvases that `cam` sees into its HDR target, fogged by
    /// `fog` (see the module docs). A no-op without any.
    pub(crate) fn draw_world_ui(
        &mut self,
        view: &mut RenderView,
        cam: &Camera,
        aspect: f32,
        fog: FogUniform,
    ) {
        let view_proj = cam.build_view_projection(aspect);
        let mut items = world_items(&view.ui, cam);
        if items.is_empty() {
            return;
        }
        items.sort_by(|a, b| b.0.total_cmp(&a.0));
        let align = self.device.limits().min_uniform_buffer_offset_alignment as usize;
        let slot = std::mem::size_of::<WorldUiUniform>();
        let stride = slot.next_multiple_of(align);
        let mut bytes = Vec::new();
        let mut draws = Vec::new();
        for &(_, i, to_world) in &items {
            let mesh = &view.ui.canvas(i).mesh;
            for (b, batch) in mesh.batches.iter().enumerate() {
                let u = WorldUiUniform {
                    view_proj: view_proj.to_cols_array(),
                    to_world: to_world.to_cols_array(),
                    eye: cam.position.extend(1.0).to_array(),
                    clip: batch.clip.map_or(NO_CLIP, |c| c.ndc(mesh.frame)),
                    fog,
                };
                draws.push((i, b, bytes.len() as u32));
                bytes.extend_from_slice(bytemuck::bytes_of(&u));
                bytes.resize(bytes.len() + stride - slot, 0);
            }
        }
        self.upload_world_uniforms(view, &bytes);
        view.ui.drawn += items.len();
        self.encode_world_ui(view, &draws);
    }

    /// Write `bytes` into the view's uniform buffer, (re)building its bind group
    /// when the buffer is new or grew.
    fn upload_world_uniforms(&self, view: &mut RenderView, bytes: &[u8]) {
        let layout = &self.ui_renderer.world.layout;
        let slot = std::mem::size_of::<WorldUiUniform>() as u64;
        let bind = |buffer: &wgpu::Buffer| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("World UI Uniforms"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(slot),
                    }),
                }],
            })
        };
        match &mut view.ui.world {
            Some(w) => {
                if w.buffer.upload(&self.device, &self.queue, bytes) {
                    w.group = bind(w.buffer.buffer());
                }
            }
            None => {
                let usage = wgpu::BufferUsages::UNIFORM;
                let mut buffer = GrowBuffer::new(&self.device, "World UI Uniforms", usage);
                buffer.upload(&self.device, &self.queue, bytes);
                let group = bind(buffer.buffer());
                view.ui.world = Some(WorldUniforms { buffer, group });
            }
        }
    }

    /// Record and submit the world pass: `(canvas index, batch index, uniform
    /// offset)` in draw order.
    fn encode_world_ui(&self, view: &RenderView, draws: &[(usize, usize, u32)]) {
        let ui = &self.ui_renderer;
        let Some(uniforms) = view.ui.world.as_ref() else {
            return;
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("World UI Encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("World UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view.post_fx.scene_hdr.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&ui.world.pipeline);
            let mut bound = None;
            for &(i, b, offset) in draws {
                let canvas = view.ui.canvas(i);
                if bound != Some(i) {
                    pass.set_vertex_buffer(0, canvas.buffer.slice(..));
                    bound = Some(i);
                }
                let batch = &canvas.mesh.batches[b];
                pass.set_bind_group(0, ui.source_group(&batch.source), &[]);
                pass.set_bind_group(1, &uniforms.group, &[offset]);
                pass.draw(batch.range.clone(), 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
    }
}

/// `(distance, canvas index, NDC → world)` for every synced world canvas `cam`
/// renders (its culling mask includes the canvas's layer).
fn world_items(cache: &UiViewCache, cam: &Camera) -> Vec<(f32, usize, Mat4)> {
    cache
        .places()
        .enumerate()
        .filter_map(|(i, place)| {
            let CanvasSpace::World(m) = place.space else {
                return None;
            };
            if !cam.renders_layer(place.layer) {
                return None;
            }
            let half = place.size * 0.5;
            let ndc = Mat4::from_translation(half.extend(0.0)) * Mat4::from_scale(half.extend(1.0));
            let centre = m.transform_point3(half.extend(0.0));
            Some((centre.distance(cam.position), i, m * ndc))
        })
        .collect()
}

/// Where a synced canvas draws this frame — refreshed every frame, never part of
/// the re-upload check (a moving world canvas re-uploads nothing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CanvasPlace {
    /// Its reference units' space.
    pub(crate) space: CanvasSpace,
    /// Its root rect's size, reference units.
    pub(crate) size: Vec2,
    /// The canvas entity's layer (camera culling).
    pub(crate) layer: u8,
}

#[cfg(test)]
#[path = "world_tests.rs"]
mod world_tests;
