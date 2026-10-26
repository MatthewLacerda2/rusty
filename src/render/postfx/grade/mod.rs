//! The view's colour-grading LUT (#1440): the grading effects (white balance, Lift /
//! Gamma / Gain, the color adjustments, the tonemapper, the display-gamma tweak and the
//! Color Lookup, `effects::grading`) baked into one 32³ `Rgba16Float` table the
//! composite reads with a single fetch, the way Unity URP and Unreal apply a grade.
//!
//! The table is rebaked by a compute pass, assembled from the effects' grade functions
//! (`effects::assemble::bake_source`), only when its inputs change:
//! [`GradeLut::prepare`] compares this frame's packed slots with the ones it last
//! baked, so a still grade costs a frame nothing but the fetch. Each `PostFx`, so each
//! view, owns one table: two views with different grades never rebake each other's.
//! The bake pipeline ([`GradeBake`]) is the renderer's, built once for all of them,
//! with the user LUTs volumes name (`effects::grading::color_lookup`, #1441): one of
//! those is folded into the bake last, so it costs a pixel nothing.

use super::effects::grading::color_lookup::{self, Lookup, UserLuts};
use super::effects::{assemble, grade_slots, grade_slots_len, GradeCx};
use crate::components::PostProcessingComponent;
use crate::render::gpu::shaders::ShaderRegistry;

/// The bake's compute pipeline: built once, on the `Renderer`, and shared by every
/// view's [`GradeLut`], so a new view or render-texture slot compiles no shader.
/// It keeps the user LUTs too, so two views naming one file upload it once.
pub struct GradeBake {
    pipeline: wgpu::ComputePipeline,
    pub luts: UserLuts,
    /// Bound in a user LUT's place while none applies.
    neutral: wgpu::TextureView,
}

impl GradeBake {
    pub fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        assemble::register_modules(registry);
        let source = assemble::bake_source();
        let module = registry.load_source(device, &source, assemble::BAKE);
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Grading LUT Bake"),
            layout: None,
            module: &module,
            entry_point: Some("cs_bake"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            pipeline,
            luts: UserLuts::default(),
            neutral: color_lookup::neutral(device),
        }
    }
}

/// One view's grading LUT: the table, the inputs it was baked from and the bind
/// group the shared [`GradeBake`] writes it through.
pub struct GradeLut {
    /// The 3D view the composite samples (`t_lut`, binding 7); it keeps the table's
    /// texture alive.
    pub view: wgpu::TextureView,
    params_buffer: wgpu::Buffer,
    /// Made against the bake pipeline's layout, for the user LUT whose id it holds
    /// (`0`: none), and made again when that changes.
    bind_group: Option<(u64, wgpu::BindGroup)>,
    /// The packed slots the table holds now, with the user LUT's id; `None` before
    /// the first bake.
    baked: Option<(Vec<[f32; 4]>, u64)>,
}

impl GradeLut {
    /// Texels per side. Mirrors `LUT_SIZE` in `postfx/grade.wgsl`.
    pub const SIZE: u32 = 32;
    /// The bake's workgroup edge (`@workgroup_size(4, 4, 4)`).
    const GROUP: u32 = 4;

    pub fn new(device: &wgpu::Device) -> Self {
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Grading LUT Params"),
            size: (grade_slots_len() * std::mem::size_of::<[f32; 4]>()) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            view: Self::table(device),
            params_buffer,
            bind_group: None,
            baked: None,
        }
    }

    /// The empty 32³ table, written by the bake and sampled by the composite.
    fn table(device: &wgpu::Device) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Grading LUT"),
            size: wgpu::Extent3d {
                width: Self::SIZE,
                height: Self::SIZE,
                depth_or_array_layers: Self::SIZE,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Make the table hold `volume`'s grade (`None`: neutral, no tonemap), then its
    /// Color Lookup when that loads, through `bake`, baking only when they differ
    /// from what it holds: a changed knob, contribution, file or re-read file.
    /// Returns whether it baked.
    pub fn prepare(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        bake: &mut GradeBake,
        volume: Option<&PostProcessingComponent>,
    ) -> bool {
        let lookup = Lookup::of(volume);
        let user = lookup.and_then(|l| Some((bake.luts.get(device, queue, l.path)?, l)));
        let (cx, view, id) = match user {
            Some((lut, l)) => {
                let lookup = Some((lut.domain, l.contribution));
                (GradeCx { volume, lookup }, &lut.view, lut.id)
            }
            None => (
                GradeCx {
                    volume,
                    lookup: None,
                },
                &bake.neutral,
                0,
            ),
        };
        let slots = grade_slots(&cx);
        if self
            .baked
            .as_ref()
            .is_some_and(|(b, i)| same(b, &slots) && *i == id)
        {
            return false;
        }
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&slots));
        let group = match self.bind_group.take() {
            Some((i, group)) if i == id => group,
            _ => self.bind_group(device, &bake.pipeline, view),
        };
        Self::dispatch((device, queue), &bake.pipeline, &group);
        self.bind_group = Some((id, group));
        self.baked = Some((slots, id));
        true
    }

    /// The bake's bind group: the params, this table, and `user` as the user LUT.
    fn bind_group(
        &self,
        device: &wgpu::Device,
        pipeline: &wgpu::ComputePipeline,
        user: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Grading LUT Bake"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(user),
                },
            ],
        })
    }

    /// Run the bake over every texel and submit it.
    fn dispatch(
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        pipeline: &wgpu::ComputePipeline,
        group: &wgpu::BindGroup,
    ) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Grading LUT Bake"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Grading LUT Bake"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            let groups = Self::SIZE.div_ceil(Self::GROUP);
            pass.dispatch_workgroups(groups, groups, groups);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }
}

/// Byte equality: a NaN knob compares equal to itself, so it never forces a rebake
/// every frame.
fn same(a: &[[f32; 4]], b: &[[f32; 4]]) -> bool {
    bytemuck::cast_slice::<_, u8>(a) == bytemuck::cast_slice::<_, u8>(b)
}

#[cfg(test)]
pub(in crate::render::postfx) mod gpu;
