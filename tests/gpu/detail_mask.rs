//! A material's detail mask (#2258, HDRP Lit's *Detail Mask*), drawn: a lamp-lit
//! 16 m plane carries a busy detail map and a mask whose red is black on one half
//! of the main UV and white on the other. Head-on and at a grazing view, the black
//! half draws as the plane without the detail does and the white half as the plane
//! with it, unmasked; a mask without a detail map draws no detail.
//!
//! No adapter: `frame` returns `None` and the test passes without asserting.

use glam::Vec3;
use rusty::components::{DetailLayer, MaterialAsset};
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;

use crate::gpu::linear_data_maps_screenshot::scene;

const SIZE: u32 = 96;
/// The lines either side of the frame's centre left out of a half: where the
/// mask's edge falls, blurred by its mip.
const SEAM: u32 = 8;

/// A 64² PNG at a fresh path `name`, each texel `texel(x, y)`.
fn png(name: &str, texel: impl Fn(u32, u32) -> [u8; 4]) -> String {
    let path = crate::temp::dir().join(name);
    let img = image::RgbaImage::from_fn(64, 64, |x, y| image::Rgba(texel(x, y)));
    img.save(&path).expect("write png");
    path.to_string_lossy().into_owned()
}

/// The busy detail map: a checker of 8-texel cells pushing every channel far
/// from 0.5 (albedo, smoothness and both normal axes).
fn busy() -> String {
    png("rusty_2258_busy.png", |x, y| match (x / 8 + y / 8) % 2 {
        0 => [30, 40, 30, 220],
        _ => [230, 220, 230, 40],
    })
}

/// The mask: red black for the first half of `u`, white after.
fn half_mask() -> String {
    png("rusty_2258_mask.png", |x, _| match x < 32 {
        true => [0, 0, 0, 255],
        false => [255, 255, 255, 255],
    })
}

/// A grey, fairly glossy material with `detail` (map, mask), or none.
fn material(detail: Option<(Option<String>, Option<String>)>) -> MaterialAsset {
    MaterialAsset {
        base_color: [0.5, 0.5, 0.5],
        roughness: 0.4,
        detail: detail.map(|(map, mask)| DetailLayer {
            map: map.map(Into::into),
            mask: mask.map(Into::into),
            tiling: [4.0, 4.0],
            ..DetailLayer::default()
        }),
        ..MaterialAsset::default()
    }
}

/// Head-on from 12 m up, or from the plane's +Z side 20° above it. The plane's u
/// runs along world X, so the mask's edge crosses the head-on frame and splits
/// the grazing one into left and right.
fn camera(grazing: bool) -> Camera {
    match grazing {
        false => Camera::new(Vec3::new(0.0, 12.0, 0.0), 0.0, -90.0),
        true => Camera::new(
            Vec3::new(0.0, 20f32.to_radians().tan() * 9.0, 9.0),
            -90.0,
            -20.0,
        ),
    }
}

/// `m` rendered from `cam`; `None` without an adapter.
fn frame(m: MaterialAsset, cam: &Camera, name: &str) -> Option<image::RgbImage> {
    let path = crate::temp::dir().join(name);
    capture(&scene(m), cam, &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    Some(image::open(&path).expect("open").to_rgb8())
}

/// The mean absolute per-channel difference of `a` and `b` over the second half
/// (`far`) or the first of the frame's rows (`rows`) or columns, the seam left out.
fn half_diff(a: &image::RgbImage, b: &image::RgbImage, rows: bool, far: bool) -> f64 {
    let half = SIZE / 2;
    let lines = match far {
        false => 0..half - SEAM,
        true => half + SEAM..SIZE,
    };
    let (mut sum, mut n) = (0u64, 0u64);
    for i in 0..SIZE {
        for j in lines.clone() {
            let (x, y) = if rows { (i, j) } else { (j, i) };
            let (p, q) = (a.get_pixel(x, y).0, b.get_pixel(x, y).0);
            sum += p
                .iter()
                .zip(q)
                .map(|(s, t)| u64::from(s.abs_diff(t)))
                .sum::<u64>();
            n += 3;
        }
    }
    sum as f64 / n as f64
}

#[test]
fn the_masks_black_half_draws_no_detail_and_its_white_half_the_whole_detail() {
    let (map, mask) = (busy(), half_mask());
    for grazing in [false, true] {
        let cam = camera(grazing);
        let shot = |d, name: &str| {
            frame(
                material(d),
                &cam,
                &format!("rusty_2258_{grazing}_{name}.png"),
            )
        };
        let Some(plain) = shot(None, "plain") else {
            return;
        };
        let full = shot(Some((Some(map.clone()), None)), "full").expect("rendered once");
        let masked =
            shot(Some((Some(map.clone()), Some(mask.clone()))), "masked").expect("rendered");
        let lone = shot(Some((None, Some(mask.clone()))), "lone").expect("rendered");
        // The mask's black half is the frame's first: the top rows head-on, the left
        // columns at the grazing view.
        let (rows, black) = (!grazing, false);
        let half = |a, b, far| half_diff(a, b, rows, far);
        let to_plain = half(&masked, &plain, black);
        let to_full = half(&masked, &full, !black);
        let shows = half(&full, &plain, black).min(half(&full, &plain, !black));
        eprintln!("[mask] grazing {grazing}: to plain {to_plain:.2}, to full {to_full:.2}, detail shows {shows:.2}");
        assert!(shows > 4.0, "the detail shows on both halves ({shows:.2})");
        assert!(
            to_plain < 0.5,
            "the black half draws no detail ({to_plain:.2})"
        );
        assert!(
            to_full < 0.5,
            "the white half draws the whole detail ({to_full:.2})"
        );
        let lone = half(&lone, &plain, true) + half(&lone, &plain, false);
        assert!(lone < 0.5, "a mask alone draws nothing ({lone:.2})");
    }
}
