//! src/render/passes/ribbons/draw.rs — per-frame ribbon gathering and the pass.
//!
//! Gathers each visible trail (newest point first, so `t = 0` is the head at the
//! entity) and line (local points through the entity's world matrix) into one
//! vertex + index buffer, orders the ribbons back-to-front by their centroid's
//! view depth, and draws each with its blend pipeline and texture.

use std::rc::Rc;

use glam::Vec3;
use wgpu::util::DeviceExt;

use super::strip::{self, RibbonVertex};
use super::RibbonGlobals;
use crate::components::{ParticleBlend, RibbonStyle};
use crate::render::draw::sort::{back_to_front, view_depth};
use crate::render::gpu::uniforms::FogUniform;
use crate::render::passes::particles::ParticleDraws;
use crate::render::timing::GpuPass;
use crate::render::{GpuTexture, RenderView, Renderer};
use crate::scene::{Camera, Scene};

/// One ribbon's slice of the frame's index buffer and how to draw it.
struct RibbonDraw {
    blend: ParticleBlend,
    texture: Option<String>,
    first: u32,
    count: u32,
}

/// Every visible ribbon of one camera, geometry built, draws farthest first.
#[derive(Default)]
struct Ribbons {
    vertices: Vec<RibbonVertex>,
    indices: Vec<u32>,
    draws: Vec<RibbonDraw>,
}

impl Renderer {
    /// One camera's blended effects after the world canvases: ribbons, then sprite
    /// particles. Ribbon draws count toward the frame's draw calls; `instances`
    /// stays the sprite particles'.
    pub(crate) fn draw_effects(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        camera: &Camera,
    ) -> ParticleDraws {
        let ribbons = self.draw_ribbons(view, scene, camera);
        let mut draws = self.draw_particles(view, scene, camera);
        draws.draw_calls += ribbons;
        draws
    }

    /// Draw every visible trail and line into `view`'s HDR target for `camera`.
    /// Returns the draw calls issued (one per ribbon) for the frame counters.
    pub(crate) fn draw_ribbons(
        &mut self,
        view: &RenderView,
        scene: &Scene,
        camera: &Camera,
    ) -> u32 {
        // Ribbons are dynamic effects: never baked into a static probe capture.
        if self.static_capture {
            return 0;
        }
        let ribbons = collect(scene, camera);
        if ribbons.draws.is_empty() {
            return 0;
        }
        let globals = RibbonGlobals {
            view_proj: camera.build_view_projection(view.aspect()).to_cols_array(),
            cam_pos: camera.position.extend(0.0).to_array(),
            fog: FogUniform::from_settings(&scene.fog),
        };
        let rr = &self.ribbon_renderer;
        self.queue
            .write_buffer(&rr.globals_buffer, 0, bytemuck::bytes_of(&globals));
        let textures: Vec<Option<Rc<GpuTexture>>> = ribbons
            .draws
            .iter()
            .map(|d| d.texture.as_deref().map(|p| self.load_texture(p)))
            .collect();
        self.encode_ribbon_pass(view, &ribbons, &textures);
        ribbons.draws.len() as u32
    }

    /// Record the pass: load the HDR colour + depth, then each ribbon in order.
    fn encode_ribbon_pass(
        &self,
        view: &RenderView,
        ribbons: &Ribbons,
        textures: &[Option<Rc<GpuTexture>>],
    ) {
        let buffer = |label, contents: &[u8], usage| {
            let desc = wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            };
            self.device.create_buffer_init(&desc)
        };
        let vertices = buffer(
            "Ribbon Vertices",
            bytemuck::cast_slice(&ribbons.vertices),
            wgpu::BufferUsages::VERTEX,
        );
        let indices = buffer(
            "Ribbon Indices",
            bytemuck::cast_slice(&ribbons.indices),
            wgpu::BufferUsages::INDEX,
        );
        let rr = &self.ribbon_renderer;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Ribbon Encoder"),
            });
        {
            let mut pass = effect_pass(&mut encoder, view, self.gpu_timer.writes(GpuPass::Ribbons));
            pass.set_bind_group(0, &rr.globals_bind_group, &[]);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            for (draw, texture) in ribbons.draws.iter().zip(textures) {
                let tex = texture.as_deref().unwrap_or(&rr.white);
                pass.set_pipeline(rr.pipeline_for(draw.blend));
                pass.set_bind_group(1, &tex.bind_group, &[]);
                pass.draw_indexed(draw.first..draw.first + draw.count, 0, 0..1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}

/// A pass over `view`'s HDR colour and depth that keeps both (loads, never
/// clears), as every effect pass drawn over the lit scene does.
fn effect_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    view: &'e RenderView,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        multiview_mask: None,
        label: Some("Ribbon Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            depth_slice: None,
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
        timestamp_writes,
        occlusion_query_set: None,
    })
}

/// Build every ribbon `camera` sees: active, on a layer its mask keeps, with at
/// least two distinct points.
fn collect(scene: &Scene, camera: &Camera) -> Ribbons {
    let visible = |id| {
        scene.world.is_active(id)
            && crate::scene::layer_in_mask(scene.world.layer(id), camera.culling_mask)
    };
    let eye = (camera.position, camera.forward());
    let mut out = Ribbons::default();
    let mut draws = Vec::new();
    for id in scene
        .world
        .ids_with_trail()
        .into_iter()
        .filter(|&id| visible(id))
    {
        let Some(trail) = scene.world.trail(id) else {
            continue;
        };
        let head_first: Vec<Vec3> = trail.positions().rev().collect();
        draws.extend(out.push(&head_first, false, &trail.style, eye));
    }
    for id in scene
        .world
        .ids_with_line()
        .into_iter()
        .filter(|&id| visible(id))
    {
        let Some(line) = scene.world.line(id) else {
            continue;
        };
        let points = line.world_positions(scene.world_matrix(id));
        draws.extend(out.push(&points, line.looping, &line.style, eye));
    }
    back_to_front(&mut draws);
    out.draws = draws.into_iter().map(|(draw, _)| draw).collect();
    out
}

impl Ribbons {
    /// Append one ribbon's strip; returns its draw and view depth, or `None` when
    /// it has nothing to draw.
    fn push(
        &mut self,
        points: &[Vec3],
        looping: bool,
        style: &RibbonStyle,
        (cam_pos, cam_fwd): (Vec3, Vec3),
    ) -> Option<(RibbonDraw, f32)> {
        let first = self.indices.len() as u32;
        let count = strip::build(
            points,
            looping,
            style,
            &mut self.vertices,
            &mut self.indices,
        );
        if count == 0 {
            return None;
        }
        let centroid = points.iter().sum::<Vec3>() / points.len() as f32;
        let draw = RibbonDraw {
            blend: style.blend,
            texture: style.texture.clone(),
            first,
            count,
        };
        Some((draw, view_depth(centroid, cam_pos, cam_fwd)))
    }
}
