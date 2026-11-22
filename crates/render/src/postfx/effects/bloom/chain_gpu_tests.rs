use super::chain::{BloomParams, BloomTargets, CLAMP_MAX};
use crate::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::{readback, Renderer};
use rusty_components::Bloom;

const SIDE: u32 = 16;
const INF: u16 = 0x7c00;
const NAN: u16 = 0x7e00;
const NEG_INF: u16 = 0xfc00;
const TEN: u16 = 0x4900;

fn finite_non_negative(bits: u16) -> bool {
    bits & 0x7c00 != 0x7c00 && (bits & 0x8000 == 0 || bits == 0x8000)
}

pub(super) fn half(bits: u16) -> f32 {
    let (exp, man) = ((bits >> 10) & 0x1f, (bits & 0x3ff) as f32);
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let magnitude = match exp {
        0 => man * 2f32.powi(-24),
        _ => (1.0 + man / 1024.0) * 2f32.powi(exp as i32 - 15),
    };
    sign * magnitude
}

pub(super) fn scene(r: &Renderer, side: u32, lit: &[((u32, u32), u16)]) -> PfxTarget {
    let scene = PostFx::target(&r.device, side, side, "Bloom Test Scene", HDR_FORMAT);
    let mut texels = vec![0u16; (side * side * 4) as usize];
    for &((x, y), v) in lit {
        let i = ((y * side + x) * 4) as usize;
        texels[i..i + 3].fill(v);
        texels[i + 3] = 0x3c00;
    }
    r.queue.write_texture(
        scene.texture.as_image_copy(),
        bytemuck::cast_slice(&texels),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(side * 8),
            rows_per_image: None,
        },
        scene.texture.size(),
    );
    scene
}

pub(super) fn bloom(
    r: &Renderer,
    scene: &PfxTarget,
    side: u32,
    divisor: u32,
    bloom: &Bloom,
) -> BloomTargets {
    let device = &r.device;
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let passes = &r.post_passes.bloom;
    let t = BloomTargets::new(device, passes, &sampler, (side, side), divisor);
    let params = BloomParams::new(bloom, &t);
    r.queue
        .write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
    let mut encoder = device.create_command_encoder(&Default::default());
    let input = (&scene.view, &sampler);
    passes.encode(device, &mut encoder, &t, input, &r.gpu_timer);
    r.queue.submit(Some(encoder.finish()));
    t
}

pub(super) fn halves(r: &Renderer, target: &PfxTarget, size: (u32, u32)) -> Vec<u16> {
    let bytes = readback::read_texture_bytes(&r.device, &r.queue, &target.texture, size, 8);
    bytemuck::pod_collect_to_vec(&bytes)
}

#[test]
fn gpu_nan_and_inf_never_reach_the_result() {
    let Some(r) = crate::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    let corners = [
        ((3, 3), NAN),
        ((12, 3), INF),
        ((3, 12), NEG_INF),
        ((12, 12), TEN),
    ];
    let input = scene(&r, SIDE, &corners);
    let t = bloom(&r, &input, SIDE, 1, &Bloom::default());
    assert!(!t.up.is_empty(), "a chain of more than one mip");
    let halves = halves(&r, &t.up[0], t.size);
    let bad = halves.iter().filter(|&&h| !finite_non_negative(h)).count();
    assert_eq!(bad, 0, "every channel of the result is finite and ≥ 0");
    let lit = |(x, y): (u32, u32)| half(halves[((y * SIDE + x) * 4) as usize]);
    // The driver decides an inf tap: lavapipe's 0 × inf is NaN, dropped; kept, it is clamped.
    let inf = lit((12, 3));
    eprintln!("[bloom] the inf's texel: {inf}");
    assert!(
        inf < CLAMP_MAX / 4.0,
        "the inf is held under the clamp: {inf}"
    );
    assert!(lit((12, 12)) > 0.0, "the 10.0 texel glows");
}
