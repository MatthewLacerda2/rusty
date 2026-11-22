use super::outline::{bar, channel};
use crate::Renderer;

const SIZE: (u32, u32) = (1280, 720);

fn lit(roughness: f32) -> rusty_scene::Scene {
    let mut s = bar(roughness);
    let wall = s.materials.get_mut("wall").unwrap();
    wall.emissive = [1.0; 3];
    s
}

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

/// Measured: 2.3 to 5.2 on lavapipe; 17 to 20 on an RTX 2060 with texels blended by brightness.
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
