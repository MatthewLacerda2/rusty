pub(crate) mod atlas;
pub(crate) mod cascades;
mod casters;
mod clips;
mod frame;
mod layouts;
mod setup;
mod statics;
mod sweeps;
mod uniform;

use crate::gpu::ops::CountedQueue as _;
use crate::gpu::pipelines::surface::SurfaceShaders;
use crate::gpu::pipelines::winding::Facing;
use crate::gpu::pipelines::LazyPipeline;
use crate::CameraUniform;
use atlas::ShadowAtlas;
use cascades::{Cascade, MAX_CASCADES};
use casters::{CasterBuffer, ShadowCulls};
use clips::Clip;
use glam::Mat4;

pub(crate) use casters::{shadow_runs, ShadowRun};
pub(crate) use clips::{depth_only, moves, slot_clip, CasterClip, Cutout};
#[cfg(test)]
pub(crate) use layouts::{caster_entries, light_entries};
pub(crate) use setup::{depth_pipeline, lazy_depth_pipeline};
#[cfg(test)]
pub(crate) use statics::statics_key;
pub(crate) use statics::{casting_key, ObjectCasting, StaticsKey, UnbakedCasters};
pub(crate) use uniform::CascadeUniform;

/// Bytes between light-space matrices: the dynamic-offset alignment wgpu guarantees everywhere.
const LIGHT_SPACE_STRIDE: u64 = 256;

pub struct ShadowRenderer {
    static_texture: wgpu::Texture,
    active_texture: wgpu::Texture,
    static_layers: Vec<wgpu::TextureView>,
    active_layers: Vec<wgpu::TextureView>,
    pub active_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,

    pipeline: wgpu::RenderPipeline,
    time_buffer: wgpu::Buffer,
    clip_pipeline: wgpu::RenderPipeline,
    /// `[Cw, Both]`, each `[plain, clip]`.
    twins: [[LazyPipeline; 2]; 2],
    light_space_buffer: wgpu::Buffer,
    pub(crate) cascades: Vec<Cascade>,

    /// Per cascade, the statics key, light matrix and `sun_statics` it baked under; naming the
    /// scene keeps the viewport and the Inspector preview from sampling each other's bakes.
    static_cache: [Option<(StaticsKey, Mat4, bool)>; MAX_CASCADES],
    pub(crate) sun_statics: bool,

    global_bind_group: wgpu::BindGroup,
    global_layout: wgpu::BindGroupLayout,
    entity_layout: wgpu::BindGroupLayout,

    static_casters: CasterBuffer,
    dynamic_casters: CasterBuffer,
    pub(crate) drawn: Vec<(u32, u32)>,
    pub(crate) culls: ShadowCulls,
    pub(crate) instancing: bool,
    /// Whether light volumes find casters through the scene's caster index. A test hook: only
    /// `index_gpu_tests` turns it off, to compare against testing every caster.
    pub(crate) index_casters: bool,
    pub(crate) atlas: ShadowAtlas,
}

impl ShadowRenderer {
    /// Texels a side: four 1024² layers cost one 2048² map, as Unity's High tier splits it.
    pub const CASCADE_SIZE: u32 = 1024;

    pub fn new(
        device: &wgpu::Device,
        [shader, atlas_shader]: [&wgpu::ShaderModule; 2],
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
        let ([pipeline, clip_pipeline], twins) = Self::create_pipelines(device, layouts, shader);
        let static_casters = CasterBuffer::new(device, &entity_layout);
        let dynamic_casters = CasterBuffer::new(device, &entity_layout);
        let atlas_layouts = [&global_layout, &entity_layout];
        let atlas = ShadowAtlas::new(device, atlas_layouts, &time_buffer, atlas_shader);

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
            twins,
            light_space_buffer,
            cascades: Vec::new(),
            static_cache: [None; MAX_CASCADES],
            sun_statics: true,
            global_bind_group,
            global_layout,
            entity_layout,
            static_casters,
            dynamic_casters,
            drawn: Vec::new(),
            culls: ShadowCulls::default(),
            instancing: true,
            index_casters: true,
            atlas,
        }
    }

    pub(crate) fn clip_layout(
        &self,
        device: &wgpu::Device,
        material_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::PipelineLayout {
        let groups = [&self.global_layout, &self.entity_layout, material_layout];
        layouts::clip_layout(device, groups)
    }

    fn pipeline_for<'a>(
        &'a self,
        surfaces: &'a SurfaceShaders,
        clip: Option<Clip>,
        facing: Facing,
    ) -> &'a wgpu::RenderPipeline {
        let twin = |i: usize| match facing {
            Facing::Ccw => None,
            Facing::Cw => Some(self.twins[0][i].get()),
            Facing::Both => Some(self.twins[1][i].get()),
        };
        let Some(c) = clip else {
            return twin(0).unwrap_or(&self.pipeline);
        };
        match surfaces.cut(c.pipeline) {
            Some(cut) => match facing {
                Facing::Ccw => &cut.shadow,
                Facing::Cw => &cut.mirrored_shadow,
                Facing::Both => cut.two_sided_shadow.get(),
            },
            None => twin(1).unwrap_or(&self.clip_pipeline),
        }
    }

    pub(crate) fn lazy_pipelines(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.twins.iter().flatten()
    }

    #[cfg(test)]
    pub fn needs_static_bake(&self, scene: &rusty_scene::Scene) -> bool {
        let key = statics_key(scene);
        let count = self.cascades.len().max(1);
        self.static_cache[..count]
            .iter()
            .any(|baked| baked.map(|(k, _, _)| k) != Some(key))
    }

    pub fn invalidate_static_cache(&mut self) {
        self.static_cache = [None; MAX_CASCADES];
        self.atlas.invalidate();
    }

    pub fn set_time(&self, queue: &wgpu::Queue, time: f32) {
        let uniform = CameraUniform {
            time,
            ..bytemuck::Zeroable::zeroed()
        };
        queue.counted_write_buffer(&self.time_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn update_cascades(&mut self, queue: &wgpu::Queue, cascades: Vec<Cascade>) {
        for (i, cascade) in cascades.iter().enumerate() {
            queue.counted_write_buffer(
                &self.light_space_buffer,
                i as u64 * LIGHT_SPACE_STRIDE,
                bytemuck::bytes_of(&cascade.light_space.to_cols_array()),
            );
        }
        self.cascades = cascades;
    }
}

#[cfg(test)]
mod cache_tests;

#[cfg(test)]
mod casting_tests;

#[cfg(test)]
mod face_tests;

#[cfg(test)]
mod cutout_tests;

#[cfg(test)]
pub(super) mod contact_tests;

#[cfg(test)]
mod live_sun_tests;
#[cfg(test)]
mod mapped_cutout_tests;
#[cfg(test)]
mod mirror_tests;
