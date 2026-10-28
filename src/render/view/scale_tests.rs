//! The render scale (#1498): a scaled view sizes its scene targets by the scale and
//! keeps its output at the surface's size, and its upscaled frame is the native one,
//! only softer. GPU tests skip with no adapter.

use glam::Vec3;

use super::{scene_size, PixelSize, RenderScale, RenderView};
use crate::render::{readback, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

#[test]
fn the_scene_size_is_the_output_at_the_scale_and_native_is_the_output() {
    let retina = PixelSize::new(2940, 1388);
    assert_eq!(scene_size(RenderScale::NATIVE, retina, MAX), retina);
    let half = scene_size(RenderScale::new(0.5), retina, MAX);
    assert_eq!(half, PixelSize::new(1470, 694));
    let tiny = scene_size(RenderScale::new(0.1), PixelSize::new(3, 3), MAX);
    assert_eq!(tiny, PixelSize::new(1, 1));
}

/// `wgpu::Limits::default().max_texture_dimension_2d`.
const MAX: u32 = 8192;

#[test]
fn the_scene_size_is_capped_at_the_device_texture_limit_each_side() {
    // A 5K fullscreen player at scale 2 would ask for 10240 px.
    let five_k = PixelSize::new(5120, 2880);
    let doubled = scene_size(RenderScale::new(2.0), five_k, MAX);
    assert_eq!(doubled, PixelSize::new(MAX, 5760));
}

#[test]
fn gpu_a_doubled_view_wider_than_the_device_limit_allocates_within_it() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(64, 48) else {
        return;
    };
    let max = r.device.limits().max_texture_dimension_2d;
    // Wider than half the limit, so scale 2 would overflow it uncapped; 1 px tall
    // keeps the allocation small.
    let width = max / 2 + 64;
    let mut view = RenderView::targetless(&r.device, OFFSCREEN_FORMAT, width, 1, 2);
    view.set_render_scale(RenderScale::new(2.0));
    view.resize(&r.device, width, 1, 2);
    assert_eq!(view.scene_size(), PixelSize::new(max, 2));
    assert_eq!(extent(&view.depth_texture), (max, 2));
}

fn extent(t: &wgpu::Texture) -> (u32, u32) {
    (t.width(), t.height())
}

#[test]
fn gpu_a_scaled_view_sizes_its_scene_targets_and_keeps_its_output() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(64, 48) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 64, 48, 2);
    assert!(view.scaled_output().is_none(), "native: no upscale target");
    view.set_render_scale(RenderScale::new(0.5));
    view.resize(&r.device, 64, 48, 2);
    assert_eq!(view.size(), PixelSize::new(64, 48));
    assert_eq!(view.scene_size(), PixelSize::new(32, 24));
    assert_eq!(extent(&view.depth_texture), (32, 24));
    assert_eq!(view.post_fx.full_size, (32, 24));
    assert_eq!(extent(view.color_target().unwrap()), (64, 48));
    assert!(view.scaled_output().is_some());
    // Back to native: one size again, and no upscale target.
    view.set_render_scale(RenderScale::NATIVE);
    view.resize(&r.device, 64, 48, 2);
    assert_eq!(view.scene_size(), view.size());
    assert_eq!(extent(&view.depth_texture), (64, 48));
    assert!(view.scaled_output().is_none());
}

const W: u32 = 96;
const H: u32 = 64;

/// One frame of a lit box at `scale`, read back at the output's size.
fn frame(r: &mut Renderer, scene: &Scene, scale: f32) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H, 2);
    view.set_render_scale(RenderScale::new(scale));
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 4.0), -90.0, 0.0);
    r.render(&mut view, scene, &cam, &out, false);
    assert_eq!(view.scene_size().width, RenderScale::new(scale).apply(W));
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), W, H)
}

#[test]
fn gpu_an_upscaled_frame_is_the_native_frame_softened() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    create_entity(&mut scene, "Box", Some(Primitive::Box));
    let native = frame(&mut r, &scene, 1.0);
    for scale in [0.5, 2.0] {
        let scaled = frame(&mut r, &scene, scale);
        let diff: u64 = native
            .iter()
            .zip(&scaled)
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        let mean = diff as f64 / native.len() as f64;
        // Only the box's edges move: the frame is the same picture, filling the output.
        assert!(mean < 6.0, "scale {scale}: mean channel difference {mean}");
        let centre = ((H / 2 * W + W / 2) * 4) as usize;
        let (a, b) = (&native[centre..centre + 3], &scaled[centre..centre + 3]);
        assert!(
            a.iter().zip(b).all(|(a, b)| a.abs_diff(*b) <= 4),
            "{a:?} {b:?}"
        );
    }
}
