//! The meter's sampling on lavapipe (#1701): a frame with one tiny bright lamp,
//! moved by sub-pixel steps as a turning camera moves it, meters the same exposure,
//! because every pixel is metered by area (a ¼-resolution box downsample), where the
//! point-sampled grid before it hit the lamp or missed it.

use super::params::Step;
use super::{ExposureMeter, ExposureParams};
use crate::components::AutoExposure;
use crate::render::postfx::grade::gpu::f16;
use crate::render::postfx::HDR_FORMAT;
use crate::render::test_gpu::headless_or_skip;

const SIZE: u32 = 128;
const BACKGROUND: f32 = 0.05;
/// The lamp: 3 pixels across, at luminance 500 (10 000 times the background).
const LAMP: (f32, f32) = (3.0, 500.0);

/// The frame with the lamp's top-left corner at the centre plus `offset` pixels,
/// each pixel holding the lamp by the area of it that covers the pixel, as a
/// rasteriser's coverage does.
fn frame(offset: [f32; 2]) -> Vec<u16> {
    let (size, lum) = LAMP;
    let origin = offset.map(|o| SIZE as f32 / 2.0 + o);
    let cover = |p: u32, o: f32| ((p + 1) as f32).min(o + size) - (p as f32).max(o);
    (0..SIZE * SIZE)
        .flat_map(|i| {
            let (x, y) = (i % SIZE, i / SIZE);
            let area = cover(x, origin[0]).max(0.0) * cover(y, origin[1]).max(0.0);
            let v = BACKGROUND + lum * area;
            [v, v, v, 1.0]
        })
        .map(f16)
        .collect()
}

#[test]
fn gpu_one_tiny_bright_lamp_meters_the_same_across_sub_pixel_moves() {
    let Some(r) = headless_or_skip(SIZE, SIZE) else {
        return;
    };
    let scene = r.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Meter test"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = scene.create_view(&Default::default());
    let mut meter = ExposureMeter::new(&r.device);
    let snap = Step {
        dt: 0.0,
        snap: true,
        cut: true,
    };
    // The default window, and the window over the lit half (where the lamp sits
    // inside it, so its samples count).
    for (low, high) in [(10.0, 90.0), (50.0, 100.0)] {
        let ae = AutoExposure {
            low_percent: low,
            high_percent: high,
            ..Default::default()
        };
        let evs: Vec<f32> = (0..8)
            .map(|k| {
                let texels = frame([k as f32 / 8.0, k as f32 / 16.0]);
                r.queue.write_texture(
                    scene.as_image_copy(),
                    bytemuck::cast_slice(&texels),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(SIZE * 8),
                        rows_per_image: None,
                    },
                    scene.size(),
                );
                let params = ExposureParams::new(&ae, snap);
                meter.run(
                    (&r.device, &r.queue),
                    &r.post_passes.exposure,
                    (&view, [SIZE; 2]),
                    &params,
                );
                meter.settle(&r.device);
                meter.readout().expect("the meter read back")
            })
            .collect();
        let spread = evs.iter().fold(f32::MIN, |a, &b| a.max(b))
            - evs.iter().fold(f32::MAX, |a, &b| a.min(b));
        eprintln!("[meter] window {low}-{high}%: {evs:?}, spread {spread}");
        // The grid of point samples swung 0.018 EV here over the lit half.
        assert!(spread < 1e-3, "window {low}-{high}%: the EV moved {spread}");
    }
}
