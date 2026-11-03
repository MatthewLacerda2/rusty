//! The chain on lavapipe, fed a scene by hand (#1700): NaN, +inf and −inf texels
//! never reach the result, whatever a driver does when the forward pass overflows
//! a half float, and an inf still glows, clamped and tamed.

use super::chain::{BloomParams, BloomTargets};
use crate::components::Bloom;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::{readback, Renderer};

const SIDE: u32 = 16;
/// Half-float bits: +inf, a quiet NaN, −inf and 10.0.
const INF: u16 = 0x7c00;
const NAN: u16 = 0x7e00;
const NEG_INF: u16 = 0xfc00;
const TEN: u16 = 0x4900;

/// Whether half-float `bits` is finite and not negative.
fn finite_non_negative(bits: u16) -> bool {
    bits & 0x7c00 != 0x7c00 && (bits & 0x8000 == 0 || bits == 0x8000)
}

/// A black `SIDE`² HDR scene holding a NaN, a +inf, a −inf and a 10.0 texel, one
/// near each corner.
fn scene(r: &Renderer) -> PfxTarget {
    let scene = PostFx::target(&r.device, SIDE, SIDE, "Bloom Test Scene", HDR_FORMAT);
    let mut texels = vec![0u16; (SIDE * SIDE * 4) as usize];
    let corners = [
        ((3, 3), NAN),
        ((12, 3), INF),
        ((3, 12), NEG_INF),
        ((12, 12), TEN),
    ];
    for ((x, y), v) in corners {
        let i = ((y * SIDE + x) * 4) as usize;
        texels[i..i + 3].fill(v);
        texels[i + 3] = 0x3c00;
    }
    r.queue.write_texture(
        scene.texture.as_image_copy(),
        bytemuck::cast_slice(&texels),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(SIDE * 8),
            rows_per_image: None,
        },
        scene.texture.size(),
    );
    scene
}

#[test]
fn gpu_nan_and_inf_never_reach_the_result() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    let device = &r.device;
    let scene = scene(&r);
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let t = BloomTargets::new(device, (SIDE, SIDE), 1);
    assert!(!t.up.is_empty(), "a chain of more than one mip");
    let params = BloomParams::new(&Bloom::default(), t.size);
    r.queue
        .write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
    let mut encoder = device.create_command_encoder(&Default::default());
    let passes = &r.post_passes.bloom;
    passes.encode(
        device,
        &mut encoder,
        &t,
        (&scene.view, &sampler),
        &r.gpu_timer,
    );
    r.queue.submit(Some(encoder.finish()));
    let bytes = readback::read_texture_bytes(device, &r.queue, &t.up[0].texture, t.size, 8);
    let halves: Vec<u16> = bytemuck::pod_collect_to_vec(&bytes);
    let bad = halves.iter().filter(|&&h| !finite_non_negative(h)).count();
    assert_eq!(bad, 0, "every channel of the result is finite and ≥ 0");
    let lit = |(x, y): (u32, u32)| halves[((y * SIDE + x) * 4) as usize];
    // The Karis average tames a lone texel, so the clamped inf glows like a firefly,
    // not like 65472.
    assert!(lit((12, 3)) > 0, "the inf texel glows, clamped");
    assert!(lit((12, 12)) > 0, "the 10.0 texel glows");
}
