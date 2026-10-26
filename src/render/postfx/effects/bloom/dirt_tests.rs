//! The lens dirt's file (#1442) and the engine's own dirt texture, which is this
//! generator's output: `RUSTY_BLESS_LENS_DIRT=1` writes it again.

use super::cache::file;
use crate::components::rendering::post_processing::DEFAULT_DIRT_TEXTURE;

const SIDE: u32 = 256;

/// A neutral grey smudge texture: a faint film, soft blotches and small specks,
/// placed by a fixed linear congruential sequence so it never changes.
fn generate() -> image::RgbaImage {
    let mut seed = 0x1442_u64;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as f32 / (1u64 << 31) as f32
    };
    let mut spots = Vec::new();
    for i in 0..160 {
        let (radius, strength) = if i < 40 {
            (8.0 + next() * 40.0, 0.12 + next() * 0.25) // blotches
        } else {
            (0.8 + next() * 2.5, 0.35 + next() * 0.6) // specks
        };
        spots.push((next() * SIDE as f32, next() * SIDE as f32, radius, strength));
    }
    image::RgbaImage::from_fn(SIDE, SIDE, |x, y| {
        let mut v = 0.03_f32;
        for &(sx, sy, r, k) in &spots {
            let d2 = (x as f32 - sx).powi(2) + (y as f32 - sy).powi(2);
            v += k * (-d2 / (r * r)).exp();
        }
        let g = (v.min(1.0) * 255.0).round() as u8;
        image::Rgba([g, g, g, 255])
    })
}

#[test]
fn the_engine_dirt_texture_is_the_generator_output() {
    let path = crate::core::project::engine_dir()
        .join("textures")
        .join("lens_dirt.png");
    let made = generate();
    if std::env::var_os("RUSTY_BLESS_LENS_DIRT").is_some() {
        made.save(&path).unwrap();
    }
    let shipped = image::open(&path)
        .expect("engine/textures/lens_dirt.png")
        .to_rgba8();
    assert_eq!(shipped.dimensions(), made.dimensions());
    // ±1 per channel: `f32::exp` may differ by an ulp between glibc and Apple's libm.
    let near = (shipped.as_raw().iter().zip(made.as_raw())).all(|(a, b)| a.abs_diff(*b) <= 1);
    assert!(near, "re-bless with RUSTY_BLESS_LENS_DIRT=1");
}

#[test]
fn the_default_dirt_reads_from_the_engine_folder_and_a_path_as_it_is() {
    let want = crate::core::project::engine_path("textures/lens_dirt.png");
    assert_eq!(file(DEFAULT_DIRT_TEXTURE).as_str(), want);
    assert_eq!(file("assets/dirt.png").as_str(), "assets/dirt.png");
}
