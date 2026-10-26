//! The grading LUT's GPU tests (`knobs`, `clamp`, #1440; `lookup`, #1441) and what they
//! share with the exposure meter's (#1444): the post-FX chain alone, fed a synthetic HDR
//! row. The scene target is written
//! directly, so a test chooses the exact HDR colour each pixel's grade starts from,
//! and reads the display colour the composite wrote.

mod clamp;
mod knobs;
mod lookup;

use std::collections::BTreeMap;

use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::gpu::DEPTH_FORMAT;
use crate::render::postfx::grade::{GradeParams, Lookup};
use crate::render::postfx::{PostFx, PostFxContext, PostParams, PostPasses, HDR_FORMAT};
use crate::render::timing::GpuTimer;
use crate::render::{readback, Renderer, OFFSCREEN_FORMAT};

/// Pixels in the row: one per input colour.
pub(in crate::render::postfx) const W: u32 = 64;

pub(in crate::render::postfx) struct Chain {
    pub(in crate::render::postfx) r: Renderer,
    pub(in crate::render::postfx) pfx: PostFx,
    out: wgpu::Texture,
}

/// The chain on a headless renderer, writing the sRGB format a window does, or
/// `None` without an adapter.
pub(in crate::render::postfx) fn chain() -> Option<Chain> {
    let r = crate::render::test_gpu::headless_or_skip(W, 1)?;
    let mut registry = ShaderRegistry::new("engine/shaders");
    let pfx = PostFx::new(&r.device, W, 1, OFFSCREEN_FORMAT, 2, &mut registry);
    let out = texture(&r.device, OFFSCREEN_FORMAT, wgpu::TextureUsages::COPY_SRC);
    Some(Chain { r, pfx, out })
}

impl Chain {
    /// Grade `row` (one scene-linear HDR colour per pixel) at `exposure` EV under
    /// `grade`, returning each pixel's 8-bit sRGB output.
    pub(in crate::render::postfx) fn run(
        &mut self,
        row: &[[f32; 3]],
        exposure: f32,
        grade: &GradeParams,
    ) -> Vec<[u8; 4]> {
        self.run_with(row, exposure, grade, None)
    }

    /// [`run`](Self::run), with `lookup`'s user LUT over the grade (#1441).
    pub(in crate::render::postfx) fn run_with(
        &mut self,
        row: &[[f32; 3]],
        exposure: f32,
        grade: &GradeParams,
        lookup: Option<Lookup>,
    ) -> Vec<[u8; 4]> {
        self.write_row(row);
        let (device, queue) = (&self.r.device, &self.r.queue);
        let bake = &mut self.r.grade_bake;
        self.pfx.grade.prepare((device, queue), bake, grade, lookup);
        let mut params = PostParams::default();
        params.color[0] = exposure;
        self.composite(params)
    }

    /// Make `row` (one scene-linear HDR colour per pixel) the scene the chain reads.
    pub(in crate::render::postfx) fn write_row(&mut self, row: &[[f32; 3]]) {
        assert_eq!(row.len(), W as usize);
        let texels: Vec<u16> = row
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 1.0])
            .map(f16)
            .collect();
        self.r.queue.write_texture(
            self.pfx.scene_hdr.texture.as_image_copy(),
            bytemuck::cast_slice(&texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(W * 8),
                rows_per_image: None,
            },
            self.pfx.scene_hdr.texture.size(),
        );
    }

    /// Run the chain over the scene with `params`, returning each pixel's 8-bit
    /// sRGB output.
    pub(in crate::render::postfx) fn composite(&mut self, params: PostParams) -> Vec<[u8; 4]> {
        let (device, queue) = (&self.r.device, &self.r.queue);
        let depth = texture(device, DEPTH_FORMAT, wgpu::TextureUsages::empty());
        let sky = texture(device, HDR_FORMAT, wgpu::TextureUsages::empty());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let view = |t: &wgpu::Texture| t.create_view(&Default::default());
        let (depth, sky, out) = (view(&depth), view(&sky), view(&self.out));
        let timer = GpuTimer::disabled();
        let ctx = PostFxContext {
            depth_view: &depth,
            skybox: (&sky, &sampler),
            output: &out,
            timer: &timer,
        };
        let none = BTreeMap::new();
        let passes = PostPasses {
            bloom: false,
            fxaa: false,
            custom: &[],
            post_params: &none,
        };
        self.pfx.run(device, queue, ctx, params, passes);
        let bytes = readback::read_texture_rgba8(device, queue, &self.out, W, 1);
        bytes.chunks(4).map(|p| [p[0], p[1], p[2], p[3]]).collect()
    }
}

/// A `W × 1` texture of `format`, sampled or rendered to, plus `usage`.
fn texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Grade test"),
        size: wgpu::Extent3d {
            width: W,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: usage
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

/// `v` as an IEEE half float, for the zero and positive normal values the tests
/// write (the mantissa truncated).
fn f16(v: f32) -> u16 {
    if v == 0.0 {
        return 0;
    }
    let bits = v.to_bits();
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    assert!((1..31).contains(&exp), "{v} is outside half range");
    ((exp as u32) << 10 | ((bits >> 13) & 0x3ff)) as u16
}
