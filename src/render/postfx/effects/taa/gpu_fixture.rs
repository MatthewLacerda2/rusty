//! What TAA's GPU tests share (#1702): the motion-vector fixture's renderer, scene
//! and camera (`render::motion::gpu_fixture`), with the camera's TAA on or off, and
//! a frame read back at any view size.

use glam::{Vec3, Vec4};

use super::TaaHistory;
use crate::render::motion::gpu_fixture::{scene, H, W};
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene};

pub(super) use crate::render::motion::gpu_fixture::{cube, eye, pixel, project, renderer, view};

/// The fixture's scene, its camera running TAA when `taa` (FXAA on, its default)
/// or neither anti-aliasing when `fxaa` is false too.
pub(super) fn taa_scene(taa: bool, fxaa: bool) -> (Scene, u32) {
    let (mut scene, cam) = scene(false);
    let mut c = scene.world.camera_mut(cam).unwrap();
    (c.taa_active, c.fxaa_active) = (taa, fxaa);
    drop(c);
    (scene, cam)
}

/// Render `scene` through `cam` into `view` and read the finished frame (RGBA8).
pub(super) fn frame(
    r: &mut Renderer,
    view: &mut RenderView,
    scene: &Scene,
    cam: &Camera,
) -> Vec<u8> {
    let out = view.color_target_view().unwrap();
    r.render(view, scene, cam, &out, false);
    let size = view.size();
    let target = view.color_target().unwrap();
    readback::read_texture_rgba8(&r.device, &r.queue, target, size.width, size.height)
}

/// One frame of `scene` in a fresh view (no history).
pub(super) fn fresh_frame(r: &mut Renderer, scene: &Scene, cam: &Camera) -> Vec<u8> {
    let mut v = view(r);
    frame(r, &mut v, scene, cam)
}

/// `n` frames of a still `scene`; the last one.
pub(super) fn settle(
    r: &mut Renderer,
    view: &mut RenderView,
    scene: &Scene,
    cam: &Camera,
    n: u32,
) -> Vec<u8> {
    (1..n).for_each(|_| drop(frame(r, view, scene, cam)));
    frame(r, view, scene, cam)
}

/// A view `scale` times the fixture's size on each side.
pub(super) fn big_view(r: &Renderer, scale: u32) -> RenderView {
    RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W * scale, H * scale)
}

/// Whether `view` holds TAA's history (it ran this frame).
pub(super) fn holds(view: &RenderView) -> bool {
    view.post_fx.targets.get::<TaaHistory>().is_some()
}

/// The largest per-channel difference between two fixture-size frames at `(x, y)`.
pub(super) fn diff(a: &[u8], b: &[u8], (x, y): (u32, u32)) -> u8 {
    let i = ((y * W + x) * 4) as usize;
    (0..3).map(|c| a[i + c].abs_diff(b[i + c])).max().unwrap()
}

/// A unit cube at `at`, turned so its edges cross the pixel grid at an angle.
pub(super) fn tilted_cube(scene: &mut Scene, at: Vec3) -> u32 {
    let id = cube(scene, at);
    let turn = glam::Quat::from_rotation_z(0.5) * glam::Quat::from_rotation_y(0.4);
    scene.world.transform_mut(id).unwrap().rotation = turn;
    id
}

/// An sRGB byte's linear value, and back: averages of colour are taken in linear.
pub(super) fn to_linear(b: u8) -> f32 {
    let c = b as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub(super) fn to_srgb(l: f32) -> f32 {
    let c = if l <= 0.0031308 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    c * 255.0
}

/// A frame `scale` times the fixture's size, box-filtered (in linear) to the
/// fixture's: the supersampled reference, as sRGB values.
pub(super) fn downsample(big: &[u8], scale: u32) -> Vec<Vec4> {
    let (w, n) = (W * scale, (scale * scale) as f32);
    let mut out = Vec::with_capacity((W * H) as usize);
    for y in 0..H {
        for x in 0..W {
            let mut sum = Vec4::ZERO;
            for (dx, dy) in (0..scale).flat_map(|dx| (0..scale).map(move |dy| (dx, dy))) {
                let i = (((y * scale + dy) * w + x * scale + dx) * 4) as usize;
                let px = Vec4::new(
                    to_linear(big[i]),
                    to_linear(big[i + 1]),
                    to_linear(big[i + 2]),
                    0.0,
                );
                sum += px;
            }
            let mean = sum / n;
            out.push(Vec4::new(
                to_srgb(mean.x),
                to_srgb(mean.y),
                to_srgb(mean.z),
                0.0,
            ));
        }
    }
    out
}

/// Each pixel's largest channel distance from `reference`.
pub(super) fn errors(frame: &[u8], reference: &[Vec4]) -> Vec<f32> {
    let px = |i: usize| {
        Vec4::new(
            frame[i * 4] as f32,
            frame[i * 4 + 1] as f32,
            frame[i * 4 + 2] as f32,
            0.0,
        )
    };
    (0..reference.len())
        .map(|i| (px(i) - reference[i]).abs().max_element())
        .collect()
}
