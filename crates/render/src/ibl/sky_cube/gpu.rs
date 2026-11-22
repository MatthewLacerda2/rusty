use super::SKY_FACE;
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _, CountedQueue as _};
use crate::gpu::shaders::ShaderRegistry;
use crate::gpu::uniforms::ProceduralSkyUniform;
use crate::ibl::reflection_prefilter;
use crate::timing::{GpuPass, GpuTimer};
use crate::GpuTexture;

/// HDR, so a bright panorama keeps its range in a reflection.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Mirrors `@workgroup_size(8, 8, 1)` in `engine/shaders/sky/prefilter.wgsl`.
const GROUP: u32 = 8;

pub(super) fn mip_count() -> u32 {
    reflection_prefilter::mip_count(SKY_FACE)
}

pub(super) fn params(level: u32, textured: bool) -> [f32; 4] {
    let roughness = level as f32 / (mip_count() - 1) as f32;
    let size = (SKY_FACE >> level).max(1) as f32;
    let samples = reflection_prefilter::SAMPLE_COUNT as f32;
    [roughness, size, f32::from(u8::from(textured)), samples]
}

pub(super) struct Cube {
    #[cfg(test)]
    pub(super) texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    mips: Vec<wgpu::TextureView>,
}

impl Cube {
    pub(super) fn new(device: &wgpu::Device, label: &str) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: SKY_FACE,
                height: SKY_FACE,
                depth_or_array_layers: 6,
            },
            mip_level_count: mip_count(),
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(label),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let mips = (0..mip_count())
            .map(|level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(label),
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        Self {
            #[cfg(test)]
            texture,
            view,
            mips,
        }
    }
}

/// Mirrors `Params` in `engine/shaders/sky/prefilter.wgsl`: change both together.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    p: [f32; 4],
    sky: ProceduralSkyUniform,
}

pub(super) struct Filter {
    pipeline: wgpu::ComputePipeline,
    params: Vec<wgpu::Buffer>,
    black: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl Filter {
    pub(super) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let module = registry.load(device, "sky/prefilter.wgsl", "Sky Reflection Prefilter");
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Sky Reflection Prefilter"),
            layout: None,
            module: &module,
            entry_point: Some("cs_prefilter"),
            compilation_options: Default::default(),
            cache: None,
        });
        let params = (0..mip_count())
            .map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Sky Reflection Params"),
                    size: std::mem::size_of::<Params>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let black = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Sky Reflection Black Panorama"),
                size: wgpu::Extent3d::default(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sky Reflection Black Panorama"),
            ..Default::default()
        });
        Self {
            pipeline,
            params,
            black,
            sampler,
        }
    }

    pub(super) fn bake(
        &self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        timer: &GpuTimer,
        cube: &Cube,
        (panorama, sky): (Option<&GpuTexture>, &ProceduralSkyUniform),
    ) {
        let (source, sampler) = match panorama {
            Some(p) => (&p.view, &p.sampler),
            None => (&self.black, &self.sampler),
        };
        let groups: Vec<_> = (0..mip_count())
            .map(|level| {
                let size = (SKY_FACE >> level).max(1);
                let params = Params {
                    p: params(level, panorama.is_some()),
                    sky: *sky,
                };
                let buffer = &self.params[level as usize];
                queue.counted_write_buffer(buffer, 0, bytemuck::bytes_of(&params));
                let group = self.group(
                    device,
                    buffer,
                    (source, sampler),
                    &cube.mips[level as usize],
                );
                (size, group)
            })
            .collect();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Sky Reflection Prefilter"),
        });
        {
            let mut pass = encoder.counted_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Sky Reflection Prefilter"),
                timestamp_writes: timer.compute_writes(GpuPass::SkyReflection),
            });
            pass.set_pipeline(&self.pipeline);
            for (size, group) in &groups {
                pass.set_bind_group(crate::gpu::ONLY_GROUP, group, &[]);
                let n = size.div_ceil(GROUP);
                pass.dispatch_workgroups(n, n, 6);
            }
        }
        queue.counted_submit(std::iter::once(encoder.finish()));
    }

    fn group(
        &self,
        device: &wgpu::Device,
        params: &wgpu::Buffer,
        (source, sampler): (&wgpu::TextureView, &wgpu::Sampler),
        mip: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        use wgpu::BindingResource::{Sampler, TextureView};
        let resources = [
            params.as_entire_binding(),
            TextureView(source),
            Sampler(sampler),
            TextureView(mip),
        ];
        let entries: Vec<_> = (0..)
            .zip(resources)
            .map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource })
            .collect();
        device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sky Reflection Prefilter"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &entries,
        })
    }
}
