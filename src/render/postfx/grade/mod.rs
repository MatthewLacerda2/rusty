//! The view's colour-grading LUT (#1440): white balance, Lift / Gamma / Gain, hue
//! shift, the tonemapper, contrast, saturation and the display-gamma tweak, baked
//! into one 32³ `Rgba16Float` table the composite reads with a single fetch, the
//! way Unity URP and Unreal apply a grade.
//!
//! The table is rebaked by a compute pass (`postfx/lut_bake.wgsl`) only when its
//! inputs change: [`GradeLut::prepare`] compares this frame's [`GradeParams`] with
//! the ones it last baked, so a still grade costs a frame nothing but the fetch.
//! Each `PostFx`, so each view, owns one table: two views with different grades
//! never rebake each other's. The bake pipeline ([`GradeBake`]) is the renderer's,
//! built once for all of them.

mod params;

pub use params::GradeParams;

use crate::render::gpu::shaders::ShaderRegistry;

/// The bake's compute pipeline: built once, on the `Renderer`, and shared by every
/// view's [`GradeLut`], so a new view or render-texture slot compiles no shader.
pub struct GradeBake {
    pipeline: wgpu::ComputePipeline,
}

impl GradeBake {
    pub fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let module = registry.load(device, "postfx/lut_bake.wgsl", "Grading LUT Bake");
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Grading LUT Bake"),
            layout: None,
            module: &module,
            entry_point: Some("cs_bake"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self { pipeline }
    }
}

/// One view's grading LUT: the table, the inputs it was baked from and the bind
/// group the shared [`GradeBake`] writes it through.
pub struct GradeLut {
    /// The 3D view the composite samples (`t_lut`, binding 7); it keeps the table's
    /// texture alive.
    pub view: wgpu::TextureView,
    params_buffer: wgpu::Buffer,
    /// Made on the first bake, against the bake pipeline's layout.
    bind_group: Option<wgpu::BindGroup>,
    /// The inputs the table holds now; `None` before the first bake.
    baked: Option<GradeParams>,
}

impl GradeLut {
    /// Texels per side. Mirrors `LUT_SIZE` in `postfx/grade.wgsl`.
    pub const SIZE: u32 = 32;
    /// The bake's workgroup edge (`@workgroup_size(4, 4, 4)`).
    const GROUP: u32 = 4;

    pub fn new(device: &wgpu::Device) -> Self {
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Grading LUT Params"),
            size: std::mem::size_of::<GradeParams>() as u64,
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

    /// Make the table hold `params`' grade through `bake`, baking it only when they
    /// differ from the ones it holds. Returns whether it baked.
    pub fn prepare(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        bake: &GradeBake,
        params: &GradeParams,
    ) -> bool {
        if self.baked.as_ref().is_some_and(|b| same(b, params)) {
            return false;
        }
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
        let bind_group = self.bind_group.get_or_insert_with(|| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Grading LUT Bake"),
                layout: &bake.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.view),
                    },
                ],
            })
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Grading LUT Bake"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Grading LUT Bake"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&bake.pipeline);
            pass.set_bind_group(0, &*bind_group, &[]);
            let groups = Self::SIZE.div_ceil(Self::GROUP);
            pass.dispatch_workgroups(groups, groups, groups);
        }
        queue.submit(std::iter::once(encoder.finish()));
        self.baked = Some(*params);
        true
    }
}

/// Byte equality: a NaN knob compares equal to itself, so it never forces a rebake
/// every frame.
fn same(a: &GradeParams, b: &GradeParams) -> bool {
    bytemuck::bytes_of(a) == bytemuck::bytes_of(b)
}

#[cfg(test)]
mod gpu;
