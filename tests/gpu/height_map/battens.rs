//! A batten ceiling: 32 battens a repeat, `height_scale` 0.012, seen across them 60 to 75
//! degrees from the normal. The colour's dark band reaches one texel into each face, so a gap's
//! far wall draws dark; a step wider than a texel lands past it on the face instead.

use rusty::components::MaterialAsset;

use super::{frame, grazing};

fn map(name: &str, dark: fn(u32) -> bool) -> String {
    let path = crate::temp::dir().join(name);
    let img = image::RgbaImage::from_fn(256, 8, |x, _| match dark(x % 8) {
        true => image::Rgba([0, 0, 0, 255]),
        false => image::Rgba([255, 255, 255, 255]),
    });
    img.save(&path).expect("write png");
    path.to_string_lossy().into_owned()
}

pub(super) fn ceiling(height: Option<f32>, steps: (u32, u32)) -> MaterialAsset {
    let color = map("rusty_2300_batten_color.png", |x| x >= 4 || x == 0);
    let relief = map("rusty_2300_batten_height.png", |x| x >= 5);
    MaterialAsset {
        base_color_map: Some(color.into()),
        roughness: 1.0,
        tiling: [0.5, 0.5],
        height_map: height.map(|_| relief.into()),
        height_scale: height.unwrap_or(0.0),
        height_min_steps: steps.0,
        height_max_steps: steps.1,
        ..MaterialAsset::default()
    }
}

/// The share of the plane's pixels darker than half its brightest.
pub(super) fn dark_share(img: &image::RgbImage) -> f64 {
    let luma = |p: &image::Rgb<u8>| p.0.iter().map(|&c| u32::from(c)).sum::<u32>();
    let bright = img.pixels().map(luma).max().unwrap_or(0);
    let plane: Vec<u32> = img.pixels().map(luma).filter(|&l| l > 0).collect();
    let dark = plane.iter().filter(|&&l| l * 2 < bright).count();
    dark as f64 / plane.len().max(1) as f64
}

/// Dark runs down the middle column's near half: one per gap where each draws whole.
pub(super) fn runs(img: &image::RgbImage) -> usize {
    let luma = |y| {
        img.get_pixel(img.width() / 2, y)
            .0
            .iter()
            .map(|&c| u32::from(c))
            .sum::<u32>()
    };
    let bright = (0..img.height()).map(luma).max().unwrap_or(0);
    let dark: Vec<bool> = (img.height() / 2..img.height())
        .map(|y| luma(y) * 2 < bright)
        .collect();
    dark.windows(2).filter(|w| w[1] && !w[0]).count()
}

#[test]
fn raised_steps_draw_each_gap_the_default_skips() {
    for elevation in [30.0, 20.0, 15.0] {
        let cam = grazing(elevation);
        let name = |what: &str| format!("rusty_2300_{what}_{elevation}.png");
        let flat = frame(ceiling(None, (5, 15)), &cam, &name("flat"));
        let default = frame(ceiling(Some(0.012), (5, 15)), &cam, &name("default"));
        let raised = frame(ceiling(Some(0.012), (32, 64)), &cam, &name("raised"));
        let (Some(flat), Some(default), Some(raised)) = (flat, default, raised) else {
            eprintln!("[height-map] no GPU adapter — skipping");
            return;
        };
        let kept = |img| dark_share(img) / dark_share(&flat).max(1e-6);
        let (lost, held) = (kept(&default), kept(&raised));
        let (flat_runs, lost_runs, held_runs) = (runs(&flat), runs(&default), runs(&raised));
        let angle = 90.0 - elevation;
        eprintln!(
            "[height-map] {angle}°: gap kept, default {lost:.3}, raised {held:.3}; \
             near runs flat {flat_runs}, default {lost_runs}, raised {held_runs}"
        );
        // Measured on lavapipe: kept 0.849/0.962 at 60°, 0.856/0.911 at 70°, 0.740/0.880 at
        // 75°; runs 3/3/3, 4/4/4, 4/6/4 (flat/default/raised).
        assert!(
            held - lost > 0.03,
            "{angle}°: raised steps keep more gap: {lost:.3} {held:.3}"
        );
        assert!(
            held > 0.85,
            "{angle}°: raised steps keep the gaps: {held:.3}"
        );
        assert_eq!(held_runs, flat_runs, "{angle}°: each near gap drawn once");
        if angle == 75.0 {
            assert!(
                lost_runs > flat_runs,
                "today's 5 steps split a gap: {lost_runs}"
            );
        }
    }
}
