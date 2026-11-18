//! A material's detail mask (#2258, HDRP Lit's *Detail Mask*), drawn: a lamp-lit
//! 16 m plane carries a busy detail map and a mask whose red is black on one half
//! of the main UV and white on the other. Head-on and at a grazing view, the black
//! half draws as the plane without the detail does and the white half as the plane
//! with it, unmasked; a mask without a detail map draws no detail, and a missing
//! mask file applies the detail everywhere. No adapter: `frame` returns `None`.

use glam::Vec3;
use rusty::components::{DetailLayer, MaterialAsset};
use rusty::dev::{capture::CaptureHost, screenshot::capture_into};
use rusty::scene::Camera;

use crate::gpu::linear_data_maps_screenshot::scene;

const SIZE: u32 = 96;
/// The lines either side of the centre left out: the mask's edge, blurred by its mip.
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
    png("rusty_2258_mask.png", |x, _| [u8::from(x >= 32) * 255; 4])
}

/// A grey, fairly glossy material with `detail` (map, mask), or none.
fn material(detail: Detail) -> MaterialAsset {
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

/// A detail layer's (map, mask), or none.
type Detail = Option<(Option<String>, Option<String>)>;

/// `material(d)` from `camera(grazing)` through `h`, one renderer for every shot (one
/// each was most of the test's time on lavapipe, #2350); `None` without an adapter.
fn frame(h: &mut CaptureHost, grazing: bool, d: Detail, name: &str) -> Option<image::RgbImage> {
    let path = crate::temp::dir().join(format!("rusty_2258_{grazing}_{name}.png"));
    capture_into(h, &scene(material(d)), &camera(grazing), &path, SIZE, SIZE)
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
    let (map, mask) = (Some(busy()), Some(half_mask()));
    let missing = Some("rusty_2258_no_such_mask.png".to_string());
    let mut host = CaptureHost::new();
    // Where the time goes, for a desktop run to read: on lavapipe the first shot is 1.4
    // of 1.6 s (9.0 of 9.5 with Mesa's shader cache cold), each later one 10 to 190 ms.
    let (start, mut first) = (std::time::Instant::now(), None);
    for grazing in [false, true] {
        let mut shot = |d, name| frame(&mut host, grazing, d, name);
        let Some(plain) = shot(None, "plain") else {
            return;
        };
        first.get_or_insert_with(|| start.elapsed());
        let full = shot(Some((map.clone(), None)), "full").expect("rendered");
        let masked = shot(Some((map.clone(), mask.clone())), "masked").expect("rendered");
        let lone = shot(Some((None, mask.clone())), "lone").expect("rendered");
        let unread = shot(Some((map.clone(), missing.clone())), "missing").expect("rendered");
        // The mask's black half is the frame's first: the top rows head-on, the left
        // columns at the grazing view.
        let half = |a, b, far| half_diff(a, b, !grazing, far);
        let whole = |a, b| half(a, b, false) + half(a, b, true);
        let (to_plain, to_full) = (half(&masked, &plain, false), half(&masked, &full, true));
        let shows = half(&full, &plain, false).min(half(&full, &plain, true));
        let (lone, unread) = (whole(&lone, &plain), whole(&unread, &full));
        eprintln!("[mask] grazing {grazing}: to plain {to_plain:.2}, to full {to_full:.2}, detail shows {shows:.2}, lone {lone:.2}, missing {unread:.2}");
        // Measured: `shows` lavapipe 25.74 / 27.14 (head-on / grazing), Metal 26.30 /
        // 25.92, RTX 2060 (Vulkan) 24.76 / 22.71; every `< 0.5` value 0.00 on all three.
        assert!(shows > 4.0, "the detail shows on both halves ({shows:.2})");
        let near_zero = [
            (to_plain, "the black half draws no detail"),
            (to_full, "the white half draws all of it"),
            (lone, "a mask alone draws nothing"),
            (unread, "a missing mask applies it everywhere"),
        ];
        for (value, what) in near_zero {
            assert!(value < 0.5, "{what} ({value:.2})");
        }
    }
    let first = first.unwrap_or_default();
    let rest = start.elapsed() - first;
    eprintln!("[mask] first shot {first:.1?}, the rest {rest:.1?}");
}
