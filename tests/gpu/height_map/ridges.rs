//! Along a ridge the parallax shift jumps pixel to pixel, so implicit derivatives would pick a
//! far mip; `parallax::bend` reads those pixels at the unshifted derivatives. With `bend` 0 a
//! line of far mip would follow every ridge. A red/green/blue checker: only a far mip mixes all
//! three colours.

use rusty::components::MaterialAsset;

use super::{frame, grazing, stripes};

fn checker(name: &str) -> String {
    let path = crate::temp::dir().join(name);
    let img = image::RgbaImage::from_fn(64, 64, |x, y| {
        let mut c = [0, 0, 0, 255];
        c[((x / 4 + 2 * (y / 4)) % 3) as usize] = 255;
        image::Rgba(c)
    });
    img.save(&path).expect("write png");
    path.to_string_lossy().into_owned()
}

fn checkered(scale: Option<f32>) -> MaterialAsset {
    MaterialAsset {
        base_color_map: Some(checker("rusty_2301_checker.png").into()),
        roughness: 1.0,
        height_map: scale.map(|_| stripes("rusty_2301_ridge_steps.png").into()),
        height_scale: scale.unwrap_or(0.0),
        ..MaterialAsset::default()
    }
}

fn mixed(img: &image::RgbImage) -> Vec<usize> {
    let top = u32::from(img.pixels().flat_map(|p| p.0).max().unwrap_or(0));
    let all_three = |p: &&image::Rgb<u8>| p.0.iter().all(|&c| u32::from(c) * 5 > top);
    img.rows()
        .map(|row| row.filter(all_three).count())
        .collect()
}

#[test]
fn a_ridge_reads_the_mip_of_the_unshifted_surface() {
    let cam = grazing(15.0);
    let flat = frame(checkered(None), &cam, "rusty_2301_ridge_flat.png");
    let marched = frame(checkered(Some(0.05)), &cam, "rusty_2301_ridge_pom.png");
    let (Some(flat), Some(marched)) = (flat, marched) else {
        eprintln!("[height-map] no GPU adapter — skipping");
        return;
    };
    // The rows where the flat surface is magnified: under 1 in 16 of its pixels mixed.
    let (a, b) = (mixed(&flat), mixed(&marched));
    let near: Vec<usize> = (0..a.len())
        .filter(|&y| a[y] * 16 < flat.width() as usize)
        .collect();
    let share = |m: &[usize]| near.iter().map(|&y| m[y]).sum::<usize>() as f64;
    let pixels = (near.len() * flat.width() as usize).max(1) as f64;
    let (flat_share, marched_share) = (share(&a) / pixels, share(&b) / pixels);
    eprintln!("[height-map] far-mip share near: flat {flat_share:.4}, marched {marched_share:.4}");
    // Measured on lavapipe over 58 rows: flat 0.0042, marched 0.0186; `bend` 0 gives 0.0533.
    // RTX 2060, before the march tested its start: flat 0.0052, marched 0.0221.
    assert!(near.len() > 16, "the near rows are found: {}", near.len());
    assert!(
        marched_share < 0.035,
        "the ridges read no far mip: {marched_share:.4} (flat {flat_share:.4})"
    );
}
