//! Depth postpass (#1523, HDRP's *Transparent Depth Postpass*): a near, mostly
//! opaque checkered pane in front of a far wall, a wide-open lens focused on the
//! pane. Without the postpass the pane's pixels carry the wall's depth, so the pane
//! blurs with it; with it, the pane reads as in focus and stays sharp.

use glam::Vec3;
use rusty::components::{DepthOfField, DofMode, MaterialAsset};
use rusty::scene::Camera;

use super::{checker, glass, shot_at, slab, stage_with};

/// The capture size: large enough for the blur to span several pixels.
const SIZE: u32 = 160;

/// The pane 3 m from the eye over a grey wall at 25 m, its postpass `on`.
fn pane(on: bool) -> rusty::scene::Scene {
    let mut scene = stage_with(glam::Quat::from_rotation_x(-0.8), |vc| {
        vc.depth_of_field = DepthOfField {
            mode: DofMode::Bokeh,
            focus_distance: 3.0,
            aperture: 1.0,
            focal_length: 150.0,
            ..DepthOfField::default()
        }
    });
    let wall = MaterialAsset {
        base_color: [0.4; 3],
        ..MaterialAsset::default()
    };
    slab(
        &mut scene,
        "Wall",
        Vec3::new(0.0, 0.0, -20.0),
        Vec3::new(60.0, 60.0, 0.1),
        wall,
    );
    let pane = MaterialAsset {
        base_color_map: Some(checker("postpass_pane", 40).into()),
        depth_postpass: on,
        ..glass([1.0; 3], 0.9)
    };
    slab(
        &mut scene,
        "Pane",
        Vec3::new(0.0, 0.0, 2.0),
        Vec3::new(4.0, 4.0, 0.01),
        pane,
    );
    scene
}

/// How much neighbouring pixels differ across the middle of `img`, squared: a fine
/// sharp checker scores high, a blurred one (all grey) low.
fn sharpness(img: &image::RgbImage) -> u64 {
    let lum = |x, y| {
        img.get_pixel(x, y)
            .0
            .iter()
            .map(|&c| u64::from(c))
            .sum::<u64>()
    };
    let (lo, hi) = (SIZE / 4, 3 * SIZE / 4);
    let mut sum = 0;
    for y in lo..hi {
        for x in lo..hi {
            sum +=
                lum(x, y).abs_diff(lum(x + 1, y)).pow(2) + lum(x, y).abs_diff(lum(x, y + 1)).pow(2);
        }
    }
    sum
}

#[test]
fn a_depth_postpass_keeps_a_near_pane_out_of_the_background_blur() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let Some(without) = shot_at(&pane(false), &cam, "postpass_off", SIZE) else {
        return;
    };
    let with = shot_at(&pane(true), &cam, "postpass_on", SIZE).unwrap();
    let (blurred, sharp) = (sharpness(&without), sharpness(&with));
    eprintln!("[1523] pane sharpness: no postpass {blurred}, postpass {sharp}");
    assert!(
        sharp > blurred * 2,
        "the pane reads as near: {sharp} vs {blurred}"
    );
}
