//! The plasma ball's scene (#1845) and its last frame's luma.

use glam::Vec3;

use super::super::fixture::{big_view, frame, renderer, taa_scene};
use crate::components::{
    MaterialAsset, MaterialComponent, RefractionModel, RenderMode, TaaReactive,
};
use crate::core::settings::render_scale::RenderScale;
use crate::core::settings::upscaler::Upscaler;
use crate::render::motion::gpu_fixture::{H, W};
use crate::render::{RenderView, Renderer};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// The view is the fixture's this many times on a side (256×192), so the ball
/// covers some 15 pixels across.
const SCALE: u32 = 4;
/// Frames, and the ball's step per frame: 9 m/s at 60 fps.
const FRAMES: u32 = 30;
const STEP: f32 = 0.15;

/// One run of the scene.
#[derive(Clone, Copy, Debug)]
pub(super) struct Ball {
    pub taa: bool,
    pub reactive: TaaReactive,
    pub writes_motion: bool,
    /// The view's render scale: below 1 TAA upscales (TAAU), FXAA's frame is
    /// upscaled bilinearly.
    pub scale: f32,
    /// What resolves TAA's frames: TAA's own, or FSR2 in its place (#1848).
    pub upscaler: Upscaler,
}

impl Default for Ball {
    fn default() -> Self {
        Self {
            taa: true,
            reactive: TaaReactive::Auto,
            writes_motion: false,
            scale: 1.0,
            upscaler: Upscaler::Taa,
        }
    }
}

/// The view at `ball`'s render scale.
fn ball_view(r: &Renderer, ball: Ball) -> RenderView {
    let mut v = big_view(r, SCALE);
    v.set_render_scale(RenderScale::new(ball.scale));
    v.resize(&r.device, W * SCALE, H * SCALE);
    v
}

/// The ball's luma over its pixels in the last frame: the brightest and the 95th
/// percentile.
#[derive(Clone, Copy, Debug)]
pub(super) struct Stats {
    pub max: f32,
    pub p95: f32,
}

/// A 16×16 swirl of bright and dark bands in the temp dir: the detail TAA must
/// not average away.
fn swirl() -> String {
    let img = image::RgbaImage::from_fn(16, 16, |x, y| {
        let v = if (x + 2 * y) % 6 < 3 { 255 } else { 40 };
        image::Rgba([v, v, 255, 255])
    });
    let path = crate::test_temp::dir().join("rusty_1845_swirl.png");
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

/// The infinity-shout plasma ball's material, as near as primitives get: a
/// refracting transparent with a bright emissive swirl and URP's Fresnel rim.
fn plasma(ball: Ball) -> MaterialAsset {
    MaterialAsset {
        render_mode: RenderMode::Transparent,
        base_color: [0.3, 0.5, 1.0],
        alpha: 0.6,
        roughness: 0.1,
        emissive: [0.25, 0.4, 1.0],
        emissive_map: Some(swirl().into()),
        refraction_model: RefractionModel::Thin,
        reactive: ball.reactive,
        writes_motion: ball.writes_motion,
        ..MaterialAsset::default()
    }
}

/// An entity of primitive `shape` at `at`, `scale` big, drawn with `material`.
fn place(scene: &mut Scene, shape: Primitive, (at, scale): (Vec3, Vec3), material: &str) -> u32 {
    let id = create_entity(scene, material, Some(shape));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (at, scale);
    drop(t);
    let m = MaterialComponent::new(material.to_string());
    scene.world.set_material(id, Some(m));
    id
}

/// The scene, its camera's TAA as `ball.taa` asks (FXAA otherwise), and the ball.
fn build(ball: Ball) -> (Scene, u32) {
    let (mut scene, _) = taa_scene(ball.taa, true);
    let wall = MaterialAsset {
        base_color: [0.45, 0.42, 0.4],
        ..MaterialAsset::default()
    };
    scene.materials.insert("wall".to_string(), wall);
    scene.materials.insert("plasma".to_string(), plasma(ball));
    let slab = (Vec3::new(0.0, 0.0, -6.0), Vec3::new(30.0, 12.0, 0.2));
    place(&mut scene, Primitive::Box, slab, "wall");
    let sphere = (Vec3::new(-2.2, 0.0, -4.0), Vec3::splat(0.4));
    let id = place(&mut scene, Primitive::Sphere, sphere, "plasma");
    (scene, id)
}

/// Rec. 709 luma of an sRGB pixel, in bytes.
fn luma(px: &[u8]) -> f32 {
    0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32
}

/// Run `ball` for [`FRAMES`] frames and measure the last: the ball's pixels are
/// those the frame changed against the same frame without it.
pub(super) fn ball_frame(ball: Ball) -> Option<Stats> {
    let mut r = renderer()?;
    r.upscaler = ball.upscaler;
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let (mut scene, id) = build(ball);
    let mut v = ball_view(&r, ball);
    let mut last = Vec::new();
    for i in 0..FRAMES {
        scene.world.transform_mut(id).unwrap().position.x = -2.2 + STEP * i as f32;
        last = frame(&mut r, &mut v, &scene, &cam);
    }
    scene.world.set_active(id, false);
    let mut empty_view = ball_view(&r, ball);
    let empty = frame(&mut r, &mut empty_view, &scene, &cam);
    let mut lumas: Vec<f32> = last
        .chunks(4)
        .zip(empty.chunks(4))
        .filter(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .take(3)
                .any(|(x, y)| x.abs_diff(*y) > 24)
        })
        .map(|(a, _)| luma(a))
        .collect();
    assert!(lumas.len() > 40, "the ball covers {} pixels", lumas.len());
    lumas.sort_by(f32::total_cmp);
    let p95 = lumas[(lumas.len() * 95 / 100).min(lumas.len() - 1)];
    Some(Stats {
        max: *lumas.last().unwrap(),
        p95,
    })
}
