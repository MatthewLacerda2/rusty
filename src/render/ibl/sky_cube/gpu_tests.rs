//! The sky reflection cubes on a GPU (#1553): a sky change bakes once and a still
//! sky never, and the bake lays the sky out as a `texture_cube` samples it.

use glam::Vec3;

use crate::core::color::srgb_to_linear;
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene};

/// A `w × h` panorama of `texel(x, y)`, written under `name`; returns its path.
fn panorama(name: &str, (w, h): (u32, u32), texel: impl Fn(u32, u32) -> [u8; 3]) -> String {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        let [r, g, b] = texel(x, y);
        image::Rgba([r, g, b, 255])
    });
    let path = crate::test_temp::dir().join(name);
    img.save(&path).expect("write the panorama");
    path.to_string_lossy().into_owned()
}

/// Render one frame of `scene` and return how many sky cubes it baked.
fn bakes(r: &mut Renderer, view: &mut RenderView, scene: &Scene) -> u32 {
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let out = view.color_target_view().unwrap();
    r.render(view, scene, &cam, &out, false);
    r.frame_counters.sky_reflection_bakes
}

#[test]
fn gpu_a_sky_change_bakes_once_and_a_still_sky_never() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 8, 8, 2);
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    assert_eq!(
        bakes(&mut r, &mut view, &scene),
        1,
        "the first procedural frame"
    );
    assert_eq!(bakes(&mut r, &mut view, &scene), 0, "an unchanged sky");
    scene.ambient_color = Vec3::new(0.9, 0.3, 0.1);
    assert_eq!(
        bakes(&mut r, &mut view, &scene),
        0,
        "the tint is applied per sample"
    );
    let a = panorama("sky_cube_a.png", (4, 2), |_, _| [200, 40, 40]);
    let b = panorama("sky_cube_b.png", (4, 2), |_, _| [40, 40, 200]);
    scene.skybox_path = a.clone();
    assert_eq!(bakes(&mut r, &mut view, &scene), 1, "a new panorama");
    assert_eq!(bakes(&mut r, &mut view, &scene), 0, "the same panorama");
    scene.skybox_path = String::new();
    assert_eq!(
        bakes(&mut r, &mut view, &scene),
        0,
        "the procedural cube is kept"
    );
    scene.skybox_path = a;
    assert_eq!(bakes(&mut r, &mut view, &scene), 0, "so is the panorama's");
    scene.skybox_path = b;
    assert_eq!(bakes(&mut r, &mut view, &scene), 1, "another panorama");
}

/// An IEEE half to f32 (normal and zero halves: all a bake writes here).
fn half(bits: u16) -> f32 {
    let exp = i32::from((bits >> 10) & 0x1f);
    let mantissa = f32::from(bits & 0x3ff) / 1024.0;
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    match exp {
        0 => sign * mantissa * 2f32.powi(-14),
        _ => sign * (1.0 + mantissa) * 2f32.powi(exp - 15),
    }
}

/// The rgba at the centre of mip 0's `face`.
fn centre(r: &Renderer, face: u32) -> [f32; 4] {
    let (_, cube) = r.sky_reflection.panorama.as_ref().expect("a panorama cube");
    let size = super::SKY_FACE;
    let level = (&cube.texture, 0, face);
    let bytes =
        crate::render::readback::read_mip_bytes(&r.device, &r.queue, level, (size, size), 8);
    let at = ((size / 2 * size + size / 2) * 8) as usize;
    std::array::from_fn(|c| {
        half(u16::from_le_bytes([
            bytes[at + 2 * c],
            bytes[at + 2 * c + 1],
        ]))
    })
}

/// Red ramps with the panorama's u, green with its v: each face's centre direction
/// lands where `equirect_uv` puts it, in `texture_cube`'s face order (+X, -X, +Y,
/// -Y, +Z, -Z; -X sits on the seam, so is left out).
#[test]
fn gpu_the_bake_lays_the_faces_out_as_a_cube_samples_them() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 8, 8, 2);
    let mut scene = Scene::new();
    let (w, h) = (256, 128);
    scene.skybox_path = panorama("sky_cube_uv.png", (w, h), |x, y| {
        [(x * 255 / (w - 1)) as u8, (y * 255 / (h - 1)) as u8, 0]
    });
    assert_eq!(bakes(&mut r, &mut view, &scene), 1);
    // (face, u, v) of its centre direction.
    for (face, u, v) in [
        (0, 0.5, 0.5),
        (2, f32::NAN, 0.0),
        (3, f32::NAN, 1.0),
        (4, 0.75, 0.5),
        (5, 0.25, 0.5),
    ] {
        let [red, green, ..] = centre(&r, face);
        let near = |got: f32, want: f32| (got - srgb_to_linear((want * 255.0) as u8)).abs() < 0.03;
        assert!(near(green, v), "face {face}: green {green} for v {v}");
        assert!(
            u.is_nan() || near(red, u),
            "face {face}: red {red} for u {u}"
        );
    }
}
