//! What the widening leaves alone (#1702's review, item 2): it spreads a highlight
//! and blurs a reflection's mip, but a far glossy dielectric floor keeps its grazing
//! reflection and a far normal-mapped pane its opacity. Under a blue sky with no
//! lamp, a busy map's far floor reflects as much blue as its twin's, and a busy
//! glass pane over a red glowing floor lets through as much red.

use rusty::components::{MaterialAsset, RenderMode};
use rusty::scene::Scene;

use super::{floor, frames, glossy, normal_map, sky, Blue, FAR, H, W};

/// How far the busy map's far floor may be off its twin's, in each channel's mean.
const BOUND: f64 = 0.05;

/// The scene of a floor of `material` under a blue sky; with a red glowing floor
/// 0.2 m below it when `pane`.
fn scene(material: MaterialAsset, pane: bool) -> Scene {
    let mut scene = Scene::new();
    sky(&mut scene, [0, 0, 255], "rusty_1702_blue_sky.png");
    scene.ambient_intensity = 0.0;
    floor(&mut scene, "Floor", 0.0, material);
    if pane {
        let glow = MaterialAsset {
            base_color: [0.0; 3],
            emissive: [1.0, 0.0, 0.0],
            roughness: 1.0,
            ..MaterialAsset::default()
        };
        floor(&mut scene, "Glow", -0.2, glow);
    }
    scene
}

/// The mean red and blue of the lower half of the frame (the far floor), over
/// four turning frames; `None` without an adapter.
fn floor_rgb(scene: &Scene) -> Option<[f64; 2]> {
    let frames = frames(scene, FAR, 4)?;
    let lower = (W * H / 2) as usize..;
    let mut sum = [0.0; 2];
    let mut n = 0.0;
    for p in frames.iter().flat_map(|f| &f[lower.clone()]) {
        (sum[0], sum[1], n) = (sum[0] + p[0], sum[1] + p[2], n + 1.0);
    }
    Some(sum.map(|s| s / n))
}

/// A dielectric (opaque) or glass (`pane`) floor of roughness 0.08 with a busy map
/// of `blue`.
fn surface(name: &str, blue: Blue, pane: bool) -> MaterialAsset {
    let map = normal_map(name, true, blue);
    let mut m = glossy(&map, 0.0, 0.08);
    m.base_color = [0.0; 3];
    if pane {
        (m.render_mode, m.alpha) = (RenderMode::Transparent, 0.2);
    }
    m
}

#[test]
fn a_far_glossy_floor_keeps_its_grazing_reflection_and_a_pane_its_opacity() {
    for pane in [false, true] {
        let busy = surface(&format!("rusty_1702_busy_{pane}.png"), Blue::Z, pane);
        let Some(widened) = floor_rgb(&scene(busy, pane)) else {
            return;
        };
        let twin = surface(
            &format!("rusty_1702_twin_{pane}.png"),
            Blue::Pinned(255),
            pane,
        );
        let plain = floor_rgb(&scene(twin, pane)).unwrap();
        eprintln!("[toksvig] pane {pane}: red, blue {widened:.2?} vs the twin's {plain:.2?}");
        let off = |c: usize| (widened[c] / plain[c].max(1.0) - 1.0).abs();
        assert!(plain[1] > 20.0, "the floor reflects the sky: {plain:.2?}");
        assert!(
            off(1) < BOUND,
            "pane {pane}: its reflection {widened:.2?} vs {plain:.2?}"
        );
        if pane {
            assert!(plain[0] > 20.0, "the pane shows the glow: {plain:.2?}");
            assert!(off(0) < BOUND, "its opacity: {widened:.2?} vs {plain:.2?}");
        }
    }
}
