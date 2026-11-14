use glam::{Vec2, Vec4};

use super::*;

/// Total unit-square area of a triangle list.
fn area(tris: &[UnitVertex]) -> f32 {
    tris.chunks(3)
        .map(|t| (t[1].0 - t[0].0).perp_dot(t[2].0 - t[0].0).abs() * 0.5)
        .sum()
}

fn image(image_type: ImageType) -> ImageComponent {
    ImageComponent {
        image_type,
        ..Default::default()
    }
}

#[test]
fn simple_covers_the_rect_and_preserve_aspect_letterboxes() {
    let mut img = image(ImageType::Simple);
    let tris = image_triangles(&img, Vec2::new(200.0, 100.0), None);
    assert_eq!(tris.len(), 6);
    assert!((area(&tris) - 1.0).abs() < 1e-6);
    img.preserve_aspect = true;
    // A square texture in a 2:1 rect fills the middle half.
    let tris = image_triangles(&img, Vec2::new(200.0, 100.0), Some(Vec2::splat(64.0)));
    assert!((area(&tris) - 0.5).abs() < 1e-6);
    let xs: Vec<f32> = tris.iter().map(|v| v.0.x).collect();
    assert!(xs.iter().all(|&x| (0.25 - 1e-6..=0.75 + 1e-6).contains(&x)));
}

#[test]
fn sliced_keeps_borders_at_texel_size() {
    let mut img = image(ImageType::Sliced);
    img.border = Vec4::splat(10.0);
    let tris = image_triangles(&img, Vec2::new(200.0, 100.0), Some(Vec2::splat(40.0)));
    assert_eq!(tris.len(), 9 * 6);
    assert!((area(&tris) - 1.0).abs() < 1e-5);
    // The left column spans 10 of 200 units and samples 10 of 40 texels.
    let bl = tris[0..6].iter().map(|v| v.0.x).fold(0.0, f32::max);
    let bl_u = tris[0..6].iter().map(|v| v.1.x).fold(0.0, f32::max);
    assert!((bl - 0.05).abs() < 1e-6 && (bl_u - 0.25).abs() < 1e-6);
    // Without a texture it degrades to one Simple quad.
    assert_eq!(image_triangles(&img, Vec2::splat(50.0), None).len(), 6);
}

#[test]
fn tiled_repeats_at_native_size_and_crops_the_last_tile() {
    let img = image(ImageType::Tiled);
    let tris = image_triangles(&img, Vec2::new(100.0, 40.0), Some(Vec2::splat(40.0)));
    assert_eq!(tris.len(), 3 * 6, "ceil(100 / 40) × 1 tiles");
    assert!((area(&tris) - 1.0).abs() < 1e-5);
    let max_u = tris.iter().map(|v| v.1.x).fold(0.0, f32::max);
    assert!((max_u - 1.0).abs() < 1e-6);
    let huge = image_triangles(&img, Vec2::splat(1.0e5), Some(Vec2::ONE));
    assert!(
        huge.len() as f32 <= MAX_TILES * 6.0 * 1.1,
        "tile count is capped"
    );
}

#[test]
fn filled_bars_follow_origin_and_amount() {
    let mut img = image(ImageType::Filled);
    img.fill_amount = 0.5;
    let tris = image_triangles(&img, Vec2::splat(100.0), None);
    assert!((area(&tris) - 0.5).abs() < 1e-6);
    assert!(tris.iter().all(|v| v.0.x <= 0.5 && v.0 == v.1));
    img.fill_origin = FillOrigin::Right;
    let tris = image_triangles(&img, Vec2::splat(100.0), None);
    assert!(tris.iter().all(|v| v.0.x >= 0.5));
    img.fill_method = FillMethod::Vertical;
    img.fill_origin = FillOrigin::Top;
    let tris = image_triangles(&img, Vec2::splat(100.0), None);
    assert!(tris.iter().all(|v| v.0.y >= 0.5));
    img.fill_amount = 0.0;
    assert!(image_triangles(&img, Vec2::splat(100.0), None).is_empty());
}

#[test]
fn radial_sweeps_the_right_share_in_the_right_direction() {
    let mut img = image(ImageType::Filled);
    img.fill_method = FillMethod::Radial360;
    img.fill_origin = FillOrigin::Bottom;
    img.fill_amount = 0.25;
    // Clockwise from the bottom edge (y-up): through the bottom-left corner to the
    // left edge — the left-bottom quadrant.
    let tris = image_triangles(&img, Vec2::splat(100.0), None);
    assert!((area(&tris) - 0.25).abs() < 1e-5);
    assert!(tris
        .iter()
        .all(|v| v.0.x <= 0.5 + 1e-6 && v.0.y <= 0.5 + 1e-6));
    img.fill_clockwise = false;
    let tris = image_triangles(&img, Vec2::splat(100.0), None);
    assert!(tris
        .iter()
        .all(|v| v.0.x >= 0.5 - 1e-6 && v.0.y <= 0.5 + 1e-6));
    img.fill_amount = 0.625;
    let tris = image_triangles(&img, Vec2::splat(100.0), None);
    assert!((area(&tris) - 0.625).abs() < 1e-5);
    img.fill_amount = 1.0;
    assert!((area(&image_triangles(&img, Vec2::splat(9.0), None)) - 1.0).abs() < 1e-6);
}
