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
//! space instead (#619): each batch binds its slot of the view's batch uniform
//! (`effects`, shared with the overlay) carrying its clip as canvas-NDC bounds,
//! feather and Mask texture (#428), and the fragment shader fades or drops what
//! falls outside — the same space the hit-test checks masks in, so what draws is
//! what is hit. A `BackdropFilter` is not drawn here (`effects::backdrop`).

use glam::Mat4;

use super::cache::UiViewCache;
use super::effects::GroupKey;
use super::pipeline::{self, UiPass};
use crate::components::UiBlend;
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::gpu::uniforms::FogUniform;
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
    fog: FogUniform,
}

/// The world pass's shared GPU state: its uniform layout and pipeline.
pub(crate) struct WorldUiPipeline {
    pub(super) layout: wgpu::BindGroupLayout,
    /// One per blend mode, in `UiBlend::ALL` order (#425).
    pub(super) pipelines: Vec<wgpu::RenderPipeline>,
}

impl WorldUiPipeline {
    /// Build the pipelines, one per blend mode: into the HDR target, depth-tested
    /// (`LessEqual`) but never written, no culling (a canvas reads from both sides).
    /// `shared` is the overlay's groups (texture, batch); the placement is group 2.
    pub(crate) fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        shared: &[&wgpu::BindGroupLayout; 2],
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
            bind_group_layouts: &[Some(shared[0]), Some(shared[1]), Some(&layout)],
            immediate_size: 0,
        });
        let shader = ("World UI Pipeline", shader, &pipeline_layout);
        let build = |mode| pipeline::build(device, shader, UiPass::World, mode);
        let pipelines = UiBlend::ALL.into_iter().map(build).collect();
        Self { layout, pipelines }
    }
}

/// A view's world-canvas uniforms: one aligned slot per canvas, rewritten per
/// camera.
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
            let u = WorldUiUniform {
                view_proj: view_proj.to_cols_array(),
                to_world: to_world.to_cols_array(),
                eye: cam.position.extend(1.0).to_array(),
                fog,
            };
            draws.push((i, bytes.len() as u32));
            bytes.extend_from_slice(bytemuck::bytes_of(&u));
            bytes.resize(bytes.len() + stride - slot, 0);
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

    /// Record and submit the world pass: `(canvas index, placement offset)` in
    /// draw order.
    fn encode_world_ui(&self, view: &RenderView, draws: &[(usize, u32)]) {
        let ui = &self.ui_renderer;
        let Some(uniforms) = view.ui.world.as_ref() else {
            return;
        };
        let batches = |i| view.ui.canvas(i).mesh.batches.iter();
        let keys = draws
            .iter()
            .flat_map(|&(i, _)| batches(i).map(|b| GroupKey::of(b, None)));
        let groups = self.batch_groups(&view.ui.effects, keys);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("World UI Encoder"),
            });
        {
            let mut pass = world_pass(&mut encoder, view);
            let mut blend = None;
            for &(i, offset) in draws {
                let canvas = view.ui.canvas(i);
                pass.set_vertex_buffer(0, canvas.buffer.slice(..));
                pass.set_bind_group(2, &uniforms.group, &[offset]);
                for (b, batch) in canvas.mesh.batches.iter().enumerate() {
                    // A custom ui shader (#427) swaps the pipeline and adds its uniform.
                    ui.bind_batch(&mut pass, (&view.ui, i, b), UiPass::World, &mut blend);
                    let slot = view.ui.effects.batch_offset(i, b);
                    pass.set_bind_group(0, ui.source_group(&batch.source), &[]);
                    pass.set_bind_group(1, &groups[&GroupKey::of(batch, None)], &[slot]);
                    pass.draw(batch.range.clone(), 0..1);
                }
            }
        }
        self.queue.submit(Some(encoder.finish()));
    }
}

/// The world pass over `view`'s HDR scene colour and depth, both loaded.
fn world_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    view: &'e RenderView,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        multiview_mask: None,
        label: Some("World UI Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            depth_slice: None,
            view: &view.post_fx.scene_hdr.view,
            resolve_target: None,
            ops: keep(),
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &view.depth_view,
            depth_ops: Some(keep()),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

/// Load and store: draw over what the camera's passes left.
fn keep<T>() -> wgpu::Operations<T> {
    wgpu::Operations {
        load: wgpu::LoadOp::Load,
        store: wgpu::StoreOp::Store,
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

#[cfg(test)]
#[path = "world_tests.rs"]
mod world_tests;
