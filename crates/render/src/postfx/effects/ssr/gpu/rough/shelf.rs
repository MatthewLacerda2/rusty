use glam::Vec3;

use super::outline::{bar, block, channel};
use crate::Renderer;
use rusty_components::MaterialAsset;
use rusty_scene::Scene;

fn shelf(roughness: f32, mask: bool) -> Scene {
    let mut s = bar(roughness);
    for i in 0..18 {
        let tall = 0.2 + 0.1 * ((i * 7) % 4) as f32;
        let z = if i % 3 == 0 { -3.0 } else { -3.55 };
        let at = Vec3::new(-2.3 + i as f32 * 0.27, 2.2 + tall / 2.0, z);
        let size = Vec3::new(0.08, tall, 0.08);
        block(&mut s, "bottle", at, size, ([0.05; 3], 0.0, 0.6));
    }
    for x in [-1.8, -0.6, 0.6, 1.8] {
        let (at, size) = (Vec3::new(x, 3.38, -3.0), Vec3::new(0.04, 0.04, 12.0));
        block(&mut s, "rib", at, size, ([0.6; 3], 1.0, 0.35));
    }
    if mask {
        let matte = MaterialAsset {
            roughness: 1.0,
            ..MaterialAsset::default()
        };
        let lit = MaterialAsset {
            emissive: [0.0, 8.0, 0.0],
            ..matte.clone()
        };
        s.materials.insert("ceiling".into(), lit);
        s.materials.insert("rib".into(), matte);
    }
    s
}

fn steepest(red: &[f32], ceiling: &[bool], (w, h): (u32, u32), inset: u32) -> (f32, f32) {
    let open = |x: u32, y: u32| {
        let (x0, y0) = (x.saturating_sub(inset), y.saturating_sub(inset));
        let (x1, y1) = ((x + inset).min(w - 1), (y + inset).min(h - 1));
        (y0..=y1).all(|j| (x0..=x1).all(|i| ceiling[(j * w + i) as usize]))
    };
    let at = |x: u32, y: u32| red[(y * w + x) as usize];
    let mut pairs = Vec::new();
    for y in 0..h - 1 {
        for x in w / 10..w * 9 / 10 {
            if open(x, y) {
                pairs.push((at(x, y), at(x + 1, y), at(x, y + 1)));
            }
        }
    }
    let peak = pairs.iter().map(|p| p.0).fold(1e-3, f32::max);
    let step = pairs
        .iter()
        .map(|&(a, r, d)| (r - a).abs().max((d - a).abs()))
        .fold(0.0, f32::max);
    (step / peak, peak)
}

/// Measured on an RTX 2060: about 0.035; a field pulled toward the nearest hits, 0.012.
const SMOOTH: f32 = 0.025;

fn fade(r: &mut Renderer, size: (u32, u32)) -> (f32, f32) {
    let mask = shelf(0.35, true);
    let (red, green) = (channel(r, &mask, size, 0), channel(r, &mask, size, 1));
    let ceiling: Vec<bool> = red
        .iter()
        .zip(&green)
        .map(|(&r, &g)| g > 1.0 && r < 0.5)
        .collect();
    let red = channel(r, &shelf(0.35, false), size, 0);
    let (step, peak) = steepest(&red, &ceiling, size, size.1 / 90);
    (1.0 / (step * size.1 as f32), peak)
}

#[test]
fn gpu_a_satin_ceiling_reflects_bottles_at_two_depths_without_hard_blotches() {
    for size in [(640, 360), (1280, 720)] {
        let Some(mut r) = crate::test_gpu::headless_or_skip(size.0, size.1) else {
            return;
        };
        let (width, peak) = fade(&mut r, size);
        eprintln!("[ssr] at {size:?}, the satin ceiling's reflection of bottles at two depths steps at most as a fade {width:.3} of the height wide (peak red {peak:.3})");
        assert!(peak > 0.05, "the satin ceiling reflects the strip");
        assert!(
            width >= SMOOTH,
            "a satin reflection steps no harder among its hits than its blur (#2158)"
        );
    }
}
