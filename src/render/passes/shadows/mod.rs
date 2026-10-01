//! The directional-light shadow pass: cascaded shadow maps fitted to the camera (#435).
//! Each cascade is one layer of a depth-array texture. Static casters are baked into a
//! cached static array per cascade, re-baked only when that cascade's light volume
//! moves (#355); each frame the static layers are copied into the active array and the
//! dynamic casters drawn over them. Casters are drawn instanced, one draw per mesh per
//! cascade (#470, `casters`), skinned ones in their animated pose (#599). A caster
//! whose surface clips fragments — a cutout, a dissolve — clips its shadow too (#648,
//! `clips`).

pub(crate) mod cascades;
mod casters;
mod clips;
mod frame;
mod setup;
mod uniform;

use crate::render::gpu::pipelines::surface::SurfaceShaders;
use crate::render::lod::LodSelection;
use crate::render::CameraUniform;
use crate::scene::SceneId;
use cascades::{Cascade, MAX_CASCADES};
use casters::{CasterBuffer, CasterFrame};
use clips::Clip;
use glam::Mat4;

pub(crate) use setup::depth_pipeline;
pub(crate) use uniform::CascadeUniform;

/// Byte stride between the cascades' light-space matrices in the depth pass's uniform
/// buffer — the dynamic-offset alignment wgpu guarantees on every backend.
const LIGHT_SPACE_STRIDE: u64 = 256;

pub struct ShadowRenderer {
    static_texture: wgpu::Texture,
    active_texture: wgpu::Texture,
    /// One render-target view per cascade layer, static and active.
    static_layers: Vec<wgpu::TextureView>,
    active_layers: Vec<wgpu::TextureView>,
    /// The whole active array, as the forward shader samples it.
    pub active_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,

    /// Plain depth, no fragment stage: most casters.
    pipeline: wgpu::RenderPipeline,
    /// The game time a cutting variant's UV chain reads as `camera.time` (#648).
    time_buffer: wgpu::Buffer,
    /// A Cutout material's caster, alpha-tested through its material group (#648).
    clip_pipeline: wgpu::RenderPipeline,
    light_space_buffer: wgpu::Buffer,
    /// This frame's cascades, fitted to the camera by [`Self::update_cascades`].
    pub(crate) cascades: Vec<Cascade>,

    /// Per cascade: which scene's static casters are baked into that layer, and under
    /// which light volume — `None` when it holds nothing usable (#355).
    ///
    /// Naming the scene keeps two scenes rendered in one frame (the editor viewport
    /// and the Inspector preview) from sampling each other's statics; naming the
    /// light volume re-bakes a cascade exactly when it moved (the camera crossed its
    /// snap grid, or the sun turned) and never otherwise.
    static_cache: [Option<(SceneId, Mat4)>; MAX_CASCADES],

    global_bind_group: wgpu::BindGroup,
    global_layout: wgpu::BindGroupLayout,
    entity_layout: wgpu::BindGroupLayout,

    /// The static bake's and the dynamic pass's casters (#470) and joints (#599), every cascade
    /// packed into one array each — two, because both sweeps are recorded before one
    /// submit.
    static_casters: CasterBuffer,
    dynamic_casters: CasterBuffer,
    /// `(num_indices, instances)` of the caster draws since the renderer last cleared
    /// it — one frame's worth, read into the frame counters (#433).
    pub(crate) drawn: Vec<(u32, u32)>,
    /// Mirrors `Renderer::instancing`: off draws one caster per call (#470 tests).
    pub(crate) instancing: bool,
}

impl ShadowRenderer {
    /// Resolution of each cascade's layer. Four 1024² cascades are the same memory
    /// as the single 2048² map they replace (Unity's High tier splits one 2048 atlas
    /// the same way), with the first cascade covering a few metres instead of 60.
    pub const CASCADE_SIZE: u32 = 1024;

    /// The pass and its pipelines, over the forward `shader`'s shadow stage;
    /// `material_layout` is the forward pass's group 2, which a clipped caster binds
    /// (#648).
    pub fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        material_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let textures = Self::create_depth_textures(device);
        let sampler = Self::create_sampler(device);

        let light_space_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Light Space Buffer"),
            size: LIGHT_SPACE_STRIDE * MAX_CASCADES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let time_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Game Time Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (global_layout, global_bind_group, entity_layout) =
            Self::create_pass_layouts(device, [&light_space_buffer, &time_buffer]);
        let layouts = [&global_layout, &entity_layout, material_layout];
        let [pipeline, clip_pipeline] = Self::create_pipelines(device, layouts, shader);
        let static_casters = CasterBuffer::new(device, &entity_layout);
        let dynamic_casters = CasterBuffer::new(device, &entity_layout);

        Self {
            static_texture: textures.static_texture,
            active_texture: textures.active_texture,
            static_layers: textures.static_layers,
            active_layers: textures.active_layers,
            active_view: textures.active_view,
            sampler,
            pipeline,
            time_buffer,
            clip_pipeline,
            light_space_buffer,
            cascades: Vec::new(),
            static_cache: [None; MAX_CASCADES],
            global_bind_group,
            global_layout,
            entity_layout,
            static_casters,
            dynamic_casters,
            drawn: Vec::new(),
            instancing: true,
        }
    }

    /// The layout a surface variant's clipped shadow pipeline is built through (#648):
    /// this pass's two groups, then `material_layout`.
    pub(crate) fn clip_layout(
        &self,
        device: &wgpu::Device,
        material_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::PipelineLayout {
        let layouts = [&self.global_layout, &self.entity_layout, material_layout];
        setup::clip_layout(device, layouts)
    }

    /// The pipeline a run of casters draws with: plain depth, the cutout clip, or a
    /// surface variant's cut (the cutout clip if that variant failed to build).
    fn pipeline_for<'a>(
        &'a self,
        surfaces: &'a SurfaceShaders,
        clip: Option<Clip>,
    ) -> &'a wgpu::RenderPipeline {
        match clip {
            None => &self.pipeline,
            Some(c) => surfaces
                .cut(c.pipeline)
                .map_or(&self.clip_pipeline, |cut| &cut.shadow),
        }
    }

    /// Whether some cascade's static bake is not `scene`'s: it holds another scene's
    /// statics, or nothing at all.
    pub fn needs_static_bake(&self, scene: SceneId) -> bool {
        let count = self.cascades.len().max(1);
        self.static_cache[..count]
            .iter()
            .any(|baked| baked.map(|(s, _)| s) != Some(scene))
    }

    /// Drop every static bake, so the next render re-bakes them. Called when the
    /// editor changes something that moves static geometry.
    pub fn invalidate_static_cache(&mut self) {
        self.static_cache = [None; MAX_CASCADES];
    }

    /// Upload the game time, for the cuts that animate with it (#648).
    pub fn set_time(&self, queue: &wgpu::Queue, time: f32) {
        let uniform = CameraUniform {
            time,
            ..bytemuck::Zeroable::zeroed()
        };
        queue.write_buffer(&self.time_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    /// Adopt this frame's `cascades` and upload their light-space matrices.
    pub fn update_cascades(&mut self, queue: &wgpu::Queue, cascades: Vec<Cascade>) {
        for (i, cascade) in cascades.iter().enumerate() {
            queue.write_buffer(
                &self.light_space_buffer,
                i as u64 * LIGHT_SPACE_STRIDE,
                bytemuck::bytes_of(&cascade.light_space.to_cols_array()),
            );
        }
        self.cascades = cascades;
    }

    /// Record the frame's shadow sweeps: re-bake the static layers whose light volume
    /// moved, copy the static layers into the active array, and draw the dynamic
    /// casters over each cascade — skipping the LOD levels `lod` hides (#472).
    fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &CasterFrame,
        lod: &LodSelection,
    ) {
        let scene = frame.scene;
        let key = |c: &Cascade| Some((scene.id(), c.light_space));
        let stale: Vec<usize> = (0..self.cascades.len())
            .filter(|&i| self.static_cache[i] != key(&self.cascades[i]))
            .collect();
        if !stale.is_empty() {
            // The bake outlives the frame, so it cannot follow the camera's LOD
            // choice: it bakes every group at LOD0 (#472).
            let finest = LodSelection::finest(scene);
            let batches = self.prepare_casters(frame, &finest, true, &stale);
            for (&i, batches) in stale.iter().zip(&batches) {
                let view = &self.static_layers[i];
                let mut pass = depth_pass(encoder, "Shadow Static Pass", view, true);
                self.draw_casters(&mut pass, frame, batches, true, i);
            }
            for i in stale {
                self.static_cache[i] = key(&self.cascades[i]);
            }
        }

        self.copy_static_layers(encoder);

        let all: Vec<usize> = (0..self.cascades.len()).collect();
        let batches = self.prepare_casters(frame, lod, false, &all);
        for (i, batches) in batches.iter().enumerate() {
            if batches.is_empty() {
                continue;
            }
            let view = &self.active_layers[i];
            let mut pass = depth_pass(encoder, "Shadow Dynamic Pass", view, false);
            self.draw_casters(&mut pass, frame, batches, false, i);
        }
    }

    /// Copy the frame's cascade layers from the static bake into the active array.
    fn copy_static_layers(&self, encoder: &mut wgpu::CommandEncoder) {
        let layer = |texture| wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };
        encoder.copy_texture_to_texture(
            layer(&self.static_texture),
            layer(&self.active_texture),
            wgpu::Extent3d {
                width: Self::CASCADE_SIZE,
                height: Self::CASCADE_SIZE,
                depth_or_array_layers: self.cascades.len().max(1) as u32,
            },
        );
    }
}

/// A depth-only pass over one cascade layer: cleared for a bake, loaded for the
/// dynamic casters drawn over the copied statics.
fn depth_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    label: &str,
    view: &'a wgpu::TextureView,
    clear: bool,
) -> wgpu::RenderPass<'a> {
    let load = if clear {
        wgpu::LoadOp::Clear(1.0)
    } else {
        wgpu::LoadOp::Load
    };
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops: Some(wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

#[cfg(test)]
mod cache_tests;

#[cfg(test)]
mod cutout_tests;
