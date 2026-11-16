//! A lit band's fading edge draws no line (#2158, #2249): [`bar`]'s satin ceiling
//! with its back wall lit too, at the roughnesses where the band's long rays fade
//! to the probe's reflection. There the hits that find the lit wall stop and a
//! few dark ones beyond them go on, and the radius field must change no faster
//! there than inside the band: a field that let one pyramid texel outweigh the
//! next by far (a fade weight floored at 0.1, or texels blended by their
//! brightness) switched from one radius to the other within a row, drawing a
//! dark line across the band.

use super::outline::{bar, channel};
use crate::Renderer;

const SIZE: (u32, u32) = (1280, 720);

/// [`bar`] at `roughness`, its back wall glowing a dim white.
fn lit(roughness: f32) -> rusty_scene::Scene {
    let mut s = bar(roughness);
    let wall = s.materials.get_mut("wall").unwrap();
    wall.emissive = [1.0; 3];
    s
}

/// Down every column of the frame's middle, over the ceiling (its top 60%), the
/// largest bend one row makes in the reflection (its second difference), as a
/// share of the column's brightest red, times the rows squared: a smooth fade
/// bends by about as much at any size, a line by the rows squared more.
fn sharpest_bend(red: &[f32], (w, h): (u32, u32)) -> f32 {
    let column = |x: u32| (0..h * 6 / 10).map(move |y| red[(y * w + x) as usize]);
    (w * 3 / 10..w * 7 / 10)
        .map(|x| {
            let c: Vec<f32> = column(x).collect();
            let peak = c.iter().copied().fold(1e-3, f32::max);
            let bend = c
                .windows(3)
                .map(|p| (p[2] - 2.0 * p[1] + p[0]).abs())
                .fold(0.0, f32::max);
            bend / peak * (h * h) as f32 / 1000.0
        })
        .fold(0.0, f32::max)
}

/// The most a lit band may bend in one row where it fades, in [`sharpest_bend`]'s
/// units. Measured with the field as it is: 2.3 to 5.2 on lavapipe. With the field
/// weighted by brightness: 3.6 to 7.0 on an RTX 2060, 3.4 to 5.5 on lavapipe, 3.6
/// to 6.1 on Metal; 17 to 20 on the RTX 2060 with texels blended by their
/// brightness or with the fade floor at 0.1.
const BEND: f32 = 12.0;

#[test]
fn gpu_a_lit_bands_fading_edge_draws_no_line() {
    let Some(mut r): Option<Renderer> = crate::test_gpu::headless_or_skip(SIZE.0, SIZE.1) else {
        return;
    };
    for roughness in [0.39, 0.40, 0.41] {
        let bend = sharpest_bend(&channel(&mut r, &lit(roughness), SIZE, 0), SIZE);
        eprintln!(
            "[ssr] a lit band at roughness {roughness} bends at most {bend:.1} where it fades"
        );
        assert!(
            bend <= BEND,
            "a lit band's fading edge draws a line at roughness {roughness}: it bends {bend:.1}"
        );
    }
}
