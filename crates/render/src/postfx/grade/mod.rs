use super::effects::grading::color_lookup::{self, Lookup, UserLuts};
use super::effects::{assemble, grade_slots, grade_slots_len, GradeCx};
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _, CountedQueue as _};
use crate::gpu::shaders::ShaderRegistry;
use rusty_components::PostProcessingComponent;

pub struct GradeBake {
    pipeline: wgpu::ComputePipeline,
    pub luts: UserLuts,
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

pub struct GradeLut {
    pub view: wgpu::TextureView,
    params_buffer: wgpu::Buffer,
    bind_group: Option<(u64, wgpu::BindGroup)>,
    baked: Option<(Vec<[f32; 4]>, u64)>,
}

impl GradeLut {
    /// Mirrors `LUT_SIZE` in `postfx/grade.wgsl`.
    pub const SIZE: u32 = 32;
    /// Mirrors the bake's `@workgroup_size`.
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

    pub(crate) fn prepare(
        &mut self,
        (device, writes): (&wgpu::Device, crate::gpu::ops::Writes),
        bake: &mut GradeBake,
        volume: Option<&PostProcessingComponent>,
    ) -> bool {
        let queue = writes.0;
        let lookup = Lookup::of(volume);
        let user = lookup.and_then(|l| Some((bake.luts.get(device, writes, l.path)?, l)));
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
        queue.counted_write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&slots));
        let group = match self.bind_group.take() {
            Some((i, group)) if i == id => group,
            _ => self.bind_group(device, &bake.pipeline, view),
        };
        Self::dispatch((device, queue), &bake.pipeline, &group);
        self.bind_group = Some((id, group));
        self.baked = Some((slots, id));
        true
    }

    fn bind_group(
        &self,
        device: &wgpu::Device,
        pipeline: &wgpu::ComputePipeline,
        user: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.counted_bind_group(&wgpu::BindGroupDescriptor {
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

    fn dispatch(
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        pipeline: &wgpu::ComputePipeline,
        group: &wgpu::BindGroup,
    ) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Grading LUT Bake"),
        });
        {
            let mut pass = encoder.counted_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Grading LUT Bake"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(crate::gpu::ONLY_GROUP, group, &[]);
            let groups = Self::SIZE.div_ceil(Self::GROUP);
            pass.dispatch_workgroups(groups, groups, groups);
        }
        queue.counted_submit(std::iter::once(encoder.finish()));
    }
}

/// Byte equality: a NaN knob equals itself, so it never forces a rebake every frame.
fn same(a: &[[f32; 4]], b: &[[f32; 4]]) -> bool {
    bytemuck::cast_slice::<_, u8>(a) == bytemuck::cast_slice::<_, u8>(b)
}

#[cfg(test)]
pub(in crate::postfx) mod gpu;
