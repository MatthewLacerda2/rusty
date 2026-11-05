//! The sky and a translucent surface read the volume too (#1703): seen through
//! black fog (albedo 0, which only takes light) both are dimmed, and through
//! white fog lit by a lamp both gain the air's light.

use glam::Vec3;

use super::fixture::{fog, glow, lamp, open, renderer, set_fog, settled, tinted};
use crate::components::{LightMode, MaterialAsset, RenderMode, VolumetricFog};
use crate::render::Renderer;
use crate::scene::Scene;

/// A sky point above the horizon, past every froxel.
const SKY: Vec3 = Vec3::new(0.0, 4.0, -10.0);
/// Where the glass pane hangs, in front of the sky.
const PANE: Vec3 = Vec3::new(1.5, 2.5, -5.0);

/// The open room under a bright ambient (which tints the procedural sky), a
/// half-transparent white pane and a lamp between the eye and both.
fn scene() -> Scene {
    let mut scene = open(VolumetricFog::default());
    scene.ambient_color = Vec3::splat(0.6);
    tinted(
        &mut scene,
        "glass",
        (PANE, Vec3::new(1.0, 1.0, 0.02)),
        [0.8; 3],
    );
    let glass = MaterialAsset {
        base_color: [0.8; 3],
        render_mode: RenderMode::Transparent,
        alpha: 0.5,
        ..MaterialAsset::default()
    };
    scene.materials.insert("glass".into(), glass);
    lamp(
        &mut scene,
        Vec3::new(0.7, 2.6, -3.0),
        3.0,
        LightMode::Realtime,
    );
    scene
}

/// The sky's and the pane's glow under `fog`.
fn read(r: &mut Renderer, scene: &mut Scene, fog: VolumetricFog) -> [f32; 2] {
    set_fog(scene, fog);
    let frame = settled(r, scene, 4);
    [glow(&frame, SKY), glow(&frame, PANE)]
}

#[test]
fn gpu_the_sky_and_glass_are_seen_through_the_volume() {
    let Some(mut r) = renderer() else { return };
    let mut scene = scene();
    let clear = read(&mut r, &mut scene, VolumetricFog::default());
    let black = VolumetricFog {
        albedo: [0.0; 3],
        ..fog()
    };
    let dimmed = read(&mut r, &mut scene, black);
    let lit = read(&mut r, &mut scene, fog());
    eprintln!("[volumetric] sky, pane: clear {clear:?}, black {dimmed:?}, white {lit:?}");
    for (i, name) in ["the sky", "the pane"].into_iter().enumerate() {
        assert!(clear[i] > 0.5, "{name} shows: {}", clear[i]);
        assert!(
            dimmed[i] < clear[i] * 0.8,
            "black fog dims {name}: {dimmed:?} vs {clear:?}"
        );
        assert!(
            lit[i] > dimmed[i] + 0.2,
            "lit fog lights {name}: {lit:?} vs {dimmed:?}"
        );
    }
}
