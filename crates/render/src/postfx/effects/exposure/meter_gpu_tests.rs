use super::params::Step;
use super::{ExposureMeter, ExposureParams};
use crate::gpu::commands::Commands;
use crate::postfx::grade::gpu::f16;
use crate::postfx::HDR_FORMAT;
use crate::test_gpu::headless_or_skip;
use rusty_components::AutoExposure;

const BACKGROUND: f32 = 0.05;
const LAMP: (f32, f32) = (3.0, 500.0);

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
            let passes = (&r.post_passes.exposure, &r.gpu_timer);
            let mut commands = Commands::new(&r.device, &r.queue, "Meter");
            let gpu = (&r.device, &r.queue);
            meter.run(gpu, &mut commands, passes, (&view, size), &params);
            commands.send();
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
    let window = |low, high| AutoExposure {
        low_percent: low,
        high_percent: high,
        ..Default::default()
    };
    let (along, diagonal) = ([0.25, 0.0], [0.25, 0.25]);
    let cases = [
        ([128, 128], window(10.0, 90.0), along, 1e-3),
        ([128, 128], window(10.0, 90.0), diagonal, 1e-3),
        ([128, 128], window(50.0, 100.0), along, 0.06),
        ([128, 128], window(50.0, 100.0), diagonal, 0.1),
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
