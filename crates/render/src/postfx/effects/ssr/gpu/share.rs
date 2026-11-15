//! The spread pyramid keeps a lone dark hit's share of its widest window (#2249):
//! the trace stores a found hit's radius times its energy, and a dark hit's energy
//! is `ENERGY_FLOOR` (0.02) of its count, so in a coarse level the radius sum of
//! a lone dark hit with a short ray is an f16 subnormal (at level 8, the widest
//! window a 4K frame reads, for a radius under 50 pixels per unit cone at full
//! count). A GPU that flushed subnormals as it wrote a level would read that
//! window's mean radius as 0 (`postfx/ssr.wgsl`, `read_spread`). Run on each
//! adapter the GPU suite runs on, this holds that it keeps them.

use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::effects::ssr::pyramid::{Pyramid, PyramidPass};
use crate::readback::read_mip_bytes;
use rusty_core::color::{f16_to_f32, f32_to_f16};

/// The pyramid's side: its level 8 is one texel, the mean of all of level 0.
const SIDE: u32 = 256;
const WIDEST: u32 = 8;

/// The spread of one found hit at the floor, as `fs_trace` writes it: traced, a
/// radius of 10 pixels per unit cone times its energy (times `ENERGY_SCALE` 4), its
/// count 0.4 (`FOUND_FLOOR`, times `COUNT_SCALE` 64), and its energy, 0.02 of its
/// count (times `COUNT_SCALE / ENERGY_FLOOR`).
const DARK_HIT: [f32; 4] = [
    1.0,
    10.0 * 0.4 * 0.02 * 4.0,
    0.4 * 64.0,
    0.4 * 0.02 * 64.0 / 0.02,
];

/// How far each channel of the widest texel may be from the hit's 4^-8 share, as
/// a share of it: a subnormal near 5e-6 is held to about 1% by f16's last step.
const KEPT: f32 = 0.05;

#[test]
fn gpu_the_spread_pyramid_keeps_a_lone_dark_hits_share_of_its_widest_window() {
    let Some(r) = crate::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    let mut registry = ShaderRegistry::new(rusty_shadergen::engine_shader_dir());
    let pass = PyramidPass::new(&r.device, &mut registry);
    let pyramid = Pyramid::new(&r.device, &pass, (SIDE, SIDE));
    let texel: Vec<u8> = DARK_HIT
        .iter()
        .flat_map(|&c| f32_to_f16(c).to_le_bytes())
        .collect();
    r.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &pyramid.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &texel,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let mut encoder = r.device.create_command_encoder(&Default::default());
    pyramid.build(&mut encoder, &pass);
    r.queue.submit([encoder.finish()]);
    let bytes = read_mip_bytes(
        &r.device,
        &r.queue,
        (&pyramid.texture, WIDEST, 0),
        (1, 1),
        8,
    );
    let share = 0.25f32.powi(WIDEST as i32);
    for (c, want) in DARK_HIT.iter().enumerate() {
        let got = f16_to_f32(u16::from_le_bytes([bytes[c * 2], bytes[c * 2 + 1]]));
        let want = want * share;
        eprintln!(
            "[ssr] a lone dark hit's spread channel {c} at level {WIDEST}: {got:e} for {want:e}"
        );
        assert!(
            (got - want).abs() <= KEPT * want,
            "this GPU flushes the spread pyramid's subnormals: channel {c} reads {got:e}, not {want:e}"
        );
    }
}
