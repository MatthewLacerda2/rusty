//! A reflection probe's GGX prefilter on the GPU (#1602): the captured cube's source
//! mip chain (`source::source_chain`) uploaded as an `Rgba16Float` texture array,
//! every rough mip convolved in one compute pass (`ibl/probe_prefilter.wgsl`) into
//! an `Rgba16Float` storage texture, and the chain read back for the KTX2 encode.
//! `Rgba16Float` is a storage format WebGPU guarantees, so every adapter takes the
//! write (lavapipe, Metal, Vulkan and DX12 GPUs alike).
//!
//! The CPU `cpu::prefilter` (test-only) stays the reference: the shader is it
//! texel for texel (the same face matrices, source chain, per-sample mip, nearest-texel
//! lookup and sample count), and a GPU test holds the two within a written tolerance. At 512 texels
//! the CPU pass is ≈ 50 M samples on one core; here it is one compute pass.

use glam::Vec3;

use super::source::{encode_face, source_chain};
use super::{mip_count, PrefilteredCubemap, SAMPLE_COUNT, TEXEL_BYTES};
use crate::gpu::shaders::ShaderRegistry;
use crate::ibl::cube_sample::FaceProjections;
use crate::CubemapCapture;

/// The shader's workgroup edge (`@workgroup_size(8, 8, 1)`).
const GROUP: u32 = 8;

/// The compute pipeline, built once per renderer on its first probe bake.
pub(crate) struct ProbePrefilter {
    pipeline: wgpu::ComputePipeline,
}

impl ProbePrefilter {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let mut registry = ShaderRegistry::new(rusty_shadergen::engine_shader_dir());
        let label = "Reflection Probe Prefilter";
        let module = registry.load(device, "ibl/probe_prefilter.wgsl", label);
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label),
            layout: None,
            module: &module,
            entry_point: Some("cs_prefilter"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self { pipeline }
    }

    /// GGX-prefilter `capture` into a roughness mip chain, as the CPU `prefilter`
    /// does: mip 0 is the capture itself, each rougher mip convolved from it.
    pub(crate) fn run(
        &self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        capture: &CubemapCapture,
    ) -> PrefilteredCubemap {
        let base = capture.resolution.max(1);
        let levels = mip_count(base);
        let chain = source_chain(capture);
        let mut mips = vec![chain[0].each_ref().map(|face| encode_face(face))];
        if levels > 1 {
            let sizes: Vec<u32> = (1..levels).map(|level| (base >> level).max(1)).collect();
            let rough = self.filter((device, queue), (&chain, base), &sizes);
            let texel = TEXEL_BYTES as u32;
            let read = crate::readback::read_cube_mips(device, queue, (&rough, texel), &sizes);
            mips.extend(read);
        }
        PrefilteredCubemap {
            mips,
            base_size: base,
        }
    }

    /// Convolve every rough mip (`sizes`: levels 1.. of the chain) into a texture of
    /// its own, in one submitted compute pass.
    fn filter(
        &self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        (chain, base): (&[[Vec<Vec3>; 6]], u32),
        sizes: &[u32],
    ) -> wgpu::Texture {
        let source = upload(device, queue, chain, base).create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let rough = rough_texture(device, sizes);
        let projections = FaceProjections::new();
        let last = sizes.len() as u32;
        let groups: Vec<_> = (1..=last)
            .zip(sizes)
            .map(|(level, &size)| {
                let roughness = level as f32 / last as f32;
                let params = params([roughness, size as f32, base as f32], &projections);
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Reflection Probe Prefilter Params"),
                    size: (params.len() * 4) as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&params));
                let out = rough.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    base_mip_level: level - 1,
                    mip_level_count: Some(1),
                    ..Default::default()
                });
                (size, self.group(device, &buffer, &source, &out))
            })
            .collect();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Reflection Probe Prefilter"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Reflection Probe Prefilter"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            for (size, group) in &groups {
                pass.set_bind_group(0, group, &[]);
                let n = size.div_ceil(GROUP);
                pass.dispatch_workgroups(n, n, 6);
            }
        }
        queue.submit(std::iter::once(encoder.finish()));
        rough
    }

    /// One mip's bind group: its params, the captured faces and the mip's storage view.
    fn group(
        &self,
        device: &wgpu::Device,
        params: &wgpu::Buffer,
        source: &wgpu::TextureView,
        out: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        use wgpu::BindingResource::TextureView;
        let resources = [
            params.as_entire_binding(),
            TextureView(source),
            TextureView(out),
        ];
        let entries: Vec<_> = (0..)
            .zip(resources)
            .map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource })
            .collect();
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Reflection Probe Prefilter"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &entries,
        })
    }
}

/// A mip's uniform: `[roughness, mip size, capture size]` and the sample count, then
/// each face camera's view-projection and its inverse (`Params` in the shader).
fn params(p: [f32; 3], projections: &FaceProjections) -> Vec<f32> {
    let (view_proj, inverse) = projections.matrices();
    let mut out = vec![p[0], p[1], p[2], SAMPLE_COUNT as f32];
    for m in view_proj.iter().chain(inverse) {
        out.extend_from_slice(&m.to_cols_array());
    }
    out
}

/// The source chain as an `Rgba16Float` texture array, one mip per level, the base
/// `base` texels wide.
fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    chain: &[[Vec<Vec3>; 6]],
    base: u32,
) -> wgpu::Texture {
    let size = wgpu::Extent3d {
        width: base,
        height: base,
        depth_or_array_layers: 6,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Reflection Probe Capture"),
        size,
        mip_level_count: chain.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip_level, faces) in (0..).zip(chain) {
        let edge = (base >> mip_level).max(1);
        for (layer, face) in (0..).zip(faces) {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &encode_face(face),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(edge * TEXEL_BYTES as u32),
                    rows_per_image: Some(edge),
                },
                wgpu::Extent3d {
                    width: edge,
                    height: edge,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
    texture
}

/// The rough mips' target: six `Rgba16Float` layers, `sizes[0]` at its base, one
/// level per size.
fn rough_texture(device: &wgpu::Device, sizes: &[u32]) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Reflection Probe Rough Mips"),
        size: wgpu::Extent3d {
            width: sizes[0],
            height: sizes[0],
            depth_or_array_layers: 6,
        },
        mip_level_count: sizes.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

#[cfg(test)]
#[path = "gpu_tests.rs"]
mod tests;
