//! The meter's sampling on lavapipe (#1701): a frame with one tiny bright lamp,
//! swept by sub-pixel steps across block edges as a turning camera sweeps it, meters
//! a near-steady exposure, because every pixel is metered by area (a ¼-resolution box
//! downsample), where the point-sampled grid before it hit the lamp or missed it.
//! The lamp still moves the EV a little as it splits between blocks (their means
//! land in different bins), and less the larger the frame: the bounds below are
//! what the meter measures, not zero.

use super::params::Step;
use super::{ExposureMeter, ExposureParams};
use crate::postfx::grade::gpu::f16;
use crate::postfx::HDR_FORMAT;
use crate::test_gpu::headless_or_skip;
use rusty_components::AutoExposure;

const BACKGROUND: f32 = 0.05;
/// The lamp: 3 pixels across, at luminance 500 (10 000 times the background).
const LAMP: (f32, f32) = (3.0, 500.0);

/// The frame with the lamp's top-left corner at the centre plus `offset` pixels,
/// each pixel holding the lamp by the area of it that covers the pixel, as a
/// rasteriser's coverage does.
fn frame([w, h]: [u32; 2], offset: [f32; 2]) -> Vec<u16> {
    let (size, lum) = LAMP;
    let origin = [w as f32 / 2.0 + offset[0], h as f32 / 2.0 + offset[1]];
    let cover = |p: u32, o: f32| ((p + 1) as f32).min(o + size) - (p as f32).max(o);
    (0..w * h)
        .flat_map(|i| {
            let (x, y) = (i % w, i / w);
            let area = cover(x, origin[0]).max(0.0) * cover(y, origin[1]).max(0.0);
            let v = BACKGROUND + lum * area;
            [v, v, v, 1.0]
        })
        .map(f16)
        .collect()
}

/// An HDR scene of `size` the test writes.
fn texture(device: &wgpu::Device, [width, height]: [u32; 2]) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Meter test"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// The EV spread (max − min) over a sweep of the lamp by `step` pixels a frame,
/// across one whole block period (4 pixels), so it straddles a block edge.
fn spread(size: [u32; 2], ae: &AutoExposure, step: [f32; 2]) -> f32 {
    let r = headless_or_skip(size[0], size[1]).expect("checked by the caller");
    let scene = texture(&r.device, size);
    let view = scene.create_view(&Default::default());
    let mut meter = ExposureMeter::new(&r.device);
    let snap = Step {
        dt: 0.0,
        snap: true,
        cut: true,
    };
    let params = ExposureParams::new(ae, snap);
    let evs: Vec<f32> = (0..=16)
        .map(|k| {
            let at = step.map(|s| s * k as f32 - 2.0);
            let texels = frame(size, at);
            r.queue.write_texture(
                scene.as_image_copy(),
                bytemuck::cast_slice(&texels),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size[0] * 8),
                    rows_per_image: None,
                },
                scene.size(),
            );
            let passes = &r.post_passes.exposure;
            meter.run((&r.device, &r.queue), passes, (&view, size), &params);
            meter.settle(&r.device);
            meter.readout().expect("the meter read back")
        })
        .collect();
    let max = evs.iter().copied().fold(f32::MIN, f32::max);
    max - evs.iter().copied().fold(f32::MAX, f32::min)
}

#[test]
fn gpu_one_tiny_bright_lamp_meters_near_steady_across_block_edges() {
    if headless_or_skip(8, 8).is_none() {
        return;
    }
    // The default window, and the window over the lit half (where the lamp sits
    // inside it, so its samples count). Sweeps of ¼ pixel a frame: along x (one
    // block edge crossed) and diagonally (two edges crossed at once).
    let window = |low, high| AutoExposure {
        low_percent: low,
        high_percent: high,
        ..Default::default()
    };
    let (along, diagonal) = ([0.25, 0.0], [0.25, 0.25]);
    // (size, window, sweep, the bound the meter meets there, in EV)
    let cases = [
        ([128, 128], window(10.0, 90.0), along, 1e-3),
        ([128, 128], window(10.0, 90.0), diagonal, 1e-3),
        // Measured on lavapipe: 0.048 and 0.075 EV, the lamp a big share of so
        // small a frame's lit half.
        ([128, 128], window(50.0, 100.0), along, 0.06),
        ([128, 128], window(50.0, 100.0), diagonal, 0.1),
        // Measured: 0.0006 EV, the same lamp a 500th of the share at 1080p.
        ([1920, 1080], window(50.0, 100.0), diagonal, 0.002),
    ];
    for (size, ae, step, bound) in cases {
        let s = spread(size, &ae, step);
        let (low, high) = (ae.low_percent, ae.high_percent);
        eprintln!("[meter] {size:?}, window {low}-{high}%, sweep {step:?}: spread {s} EV");
        assert!(
            s <= bound,
            "{size:?} {low}-{high}% {step:?}: the EV moved {s}"
        );
    }
}
