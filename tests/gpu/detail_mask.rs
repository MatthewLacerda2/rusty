//! A material's detail mask (HDRP Lit's *Detail Mask*): a plane with a busy detail map and a mask
//! black on one half of the main UV, white on the other. Head-on and grazing, the black half
//! draws as the plane without detail, the white half with it; a mask without a detail map
//! draws none, a missing mask file applies the detail everywhere.

use glam::Vec3;
use rusty::components::{DetailLayer, MaterialAsset};
use rusty::dev::{capture::CaptureHost, screenshot::capture_into};
use rusty::scene::Camera;

use crate::gpu::linear_data_maps_screenshot::scene;

const SIZE: u32 = 96;
const SEAM: u32 = 8;

fn png(name: &str, texel: impl Fn(u32, u32) -> [u8; 4]) -> String {
    let path = crate::temp::dir().join(name);
    let img = image::RgbaImage::from_fn(64, 64, |x, y| image::Rgba(texel(x, y)));
    img.save(&path).expect("write png");
    path.to_string_lossy().into_owned()
}

fn busy() -> String {
    png("rusty_2258_busy.png", |x, y| match (x / 8 + y / 8) % 2 {
        0 => [30, 40, 30, 220],
        _ => [230, 220, 230, 40],
    })
}

fn half_mask() -> String {
    png("rusty_2258_mask.png", |x, _| [u8::from(x >= 32) * 255; 4])
}

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

type Detail = Option<(Option<String>, Option<String>)>;

fn frame(h: &mut CaptureHost, grazing: bool, d: Detail, name: &str) -> Option<image::RgbImage> {
    let path = crate::temp::dir().join(format!("rusty_2258_{grazing}_{name}.png"));
    capture_into(h, &scene(material(d)), &camera(grazing), &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    Some(image::open(&path).expect("open").to_rgb8())
}

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
    for grazing in [false, true] {
        let mut shot = |d, name| frame(&mut host, grazing, d, name);
        let Some(plain) = shot(None, "plain") else {
            return;
        };
        let full = shot(Some((map.clone(), None)), "full").expect("rendered");
        let masked = shot(Some((map.clone(), mask.clone())), "masked").expect("rendered");
        let lone = shot(Some((None, mask.clone())), "lone").expect("rendered");
        let unread = shot(Some((map.clone(), missing.clone())), "missing").expect("rendered");
        let half = |a, b, far| half_diff(a, b, !grazing, far);
        let whole = |a, b| half(a, b, false) + half(a, b, true);
        let (to_plain, to_full) = (half(&masked, &plain, false), half(&masked, &full, true));
        let shows = half(&full, &plain, false).min(half(&full, &plain, true));
        let (lone, unread) = (whole(&lone, &plain), whole(&unread, &full));
        eprintln!("[mask] grazing {grazing}: to plain {to_plain:.2}, to full {to_full:.2}, detail shows {shows:.2}, lone {lone:.2}, missing {unread:.2}");
        // Measured `shows`: lavapipe 25.74 / 27.14, Metal 26.30 / 25.92, RTX 2060 24.76 / 22.71;
        // every `< 0.5` value is 0.00 on all three.
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
}
