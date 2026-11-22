use crate::gpu::ops::{CountedEncoder as _, CountedQueue as _};
use std::rc::Rc;

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::strip::{self, RibbonVertex};
use super::RibbonGlobals;
use crate::draw::batch::sort::{back_to_front, view_depth};
use crate::gpu::uniforms::FogUniform;
use crate::passes::particles::ParticleDraws;
use crate::postfx::effects::taa;
use crate::timing::GpuPass;
use crate::{GpuTexture, RenderView, Renderer};
use rusty_asset::MapRef;
use rusty_components::{ParticleBlend, RibbonStyle};
use rusty_scene::{Camera, Scene};

const GLOBALS: u32 = 0;
const TEXTURE: u32 = 1;

struct RibbonDraw {
    blend: ParticleBlend,
    texture: Option<MapRef>,
    first: u32,
    count: u32,
}

#[derive(Default)]
struct Ribbons {
    vertices: Vec<RibbonVertex>,
    indices: Vec<u32>,
    draws: Vec<RibbonDraw>,
}

impl Renderer {
    pub(crate) fn draw_effects(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &mut RenderView,
        scene: &Scene,
        (camera, stack_index): (&Camera, f32),
        view_proj: Mat4,
    ) -> ParticleDraws {
        let ribbons = self.draw_ribbons(encoder, view, scene, (camera, stack_index), view_proj);
        let mut draws = self.draw_particles(encoder, view, scene, camera, view_proj);
        draws.draw_calls += ribbons;
        draws
    }

    pub(crate) fn draw_ribbons(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &RenderView,
        scene: &Scene,
        (camera, stack_index): (&Camera, f32),
        view_proj: Mat4,
    ) -> u32 {
        if self.capture.static_only {
            return 0;
        }
        let ribbons = collect(scene, camera);
        if ribbons.draws.is_empty() {
            return 0;
        }
        let globals = RibbonGlobals {
            view_proj: view_proj.to_cols_array(),
            cam_pos: camera.position.extend(stack_index).to_array(),
            fog: FogUniform::from_settings(&scene.fog),
        };
        let rr = &self.ribbon_renderer;
        self.queue
            .counted_write_buffer(&rr.globals_buffer, 0, bytemuck::bytes_of(&globals));
        if self.notes_due() {
            for texture in ribbons.draws.iter().filter_map(|d| d.texture.as_ref()) {
                self.note_map(scene.id(), texture.as_str());
            }
        }
        let textures: Vec<Option<Rc<GpuTexture>>> = ribbons
            .draws
            .iter()
            .map(|d| d.texture.as_ref().map(|p| self.load_texture(p)))
            .collect();
        self.encode_ribbon_pass(encoder, view, &ribbons, &textures);
        ribbons.draws.len() as u32
    }

    fn encode_ribbon_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
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
        {
            let reactive = view.post_fx.taa_reactive().is_some();
            let mut pass = effect_pass(encoder, view, self.gpu_timer.writes(GpuPass::Ribbons));
            pass.set_bind_group(GLOBALS, &rr.globals_bind_group, &[]);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            for (draw, texture) in ribbons.draws.iter().zip(textures) {
                let tex = texture.as_deref().unwrap_or(&rr.white);
                pass.set_pipeline(rr.pipeline_for(draw.blend, reactive));
                pass.set_bind_group(TEXTURE, &tex.bind_group, &[]);
                pass.draw_indexed(draw.first..draw.first + draw.count, 0, 0..1);
            }
        }
    }
}

fn effect_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    view: &'e RenderView,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) -> wgpu::RenderPass<'e> {
    encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
        multiview_mask: None,
        label: Some("Ribbon Pass"),
        color_attachments: &taa::reactive::hdr_attachments(view),
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

fn collect(scene: &Scene, camera: &Camera) -> Ribbons {
    let visible = |id| {
        scene.world.is_active_in_hierarchy(id)
            && rusty_scene::layer_in_mask(scene.world.layer(id), camera.culling_mask)
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
        let points = line.world_positions(scene.world.local_to_world(id));
        draws.extend(out.push(&points, line.looping, &line.style, eye));
    }
    back_to_front(&mut draws);
    out.draws = draws.into_iter().map(|(draw, _)| draw).collect();
    out
}

impl Ribbons {
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
