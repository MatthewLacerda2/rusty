//! A rough dielectric under a `Baked` spot (#1996): the issue's bar-stool seat
//! (black leather, albedo 0.03, roughness 0.55) under a spot 1.35 m up. Its baked
//! lobe is the live lamp's: the frame matches the same spot `Mixed`, and changing
//! the seat's roughness or albedo changes the baked frame as it changes the live one.

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};
use rusty::components::{LightComponent, LightMode, LightType, MaterialAsset};
use rusty::scene::{Camera, Scene};

use super::{bake, lamp, mean_gap, most, plane, point, shot};

/// A 3 m static seat of `albedo` and `roughness` under the issue's spot, of `mode`.
pub(super) fn seat(mode: LightMode, albedo: f32, roughness: f32) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let leather = MaterialAsset {
        base_color: [albedo; 3],
        roughness,
        ..MaterialAsset::default()
    };
    plane(&mut scene, "seat", Vec3::ZERO, Quat::IDENTITY, 0.2, leather);
    let spot = LightComponent {
        light_type: LightType::Spotlight,
        intensity: 6.0,
        range: 4.2,
        outer_cone: 90.0,
        inner_cone: 70.0,
        ..point(mode)
    };
    let down = Quat::from_rotation_x(-FRAC_PI_2);
    lamp(&mut scene, Vec3::Y * 1.35, down, spot);
    scene
}

/// The seat from a standing guest's side, the spot's mirror image in view.
fn camera() -> Camera {
    Camera::new(Vec3::new(-1.5, 1.0, 0.0), 0.0, -45.0)
}

/// The seat with its spot `Baked` (a directional bake: its lobe) and `Mixed` (a
/// plain bake: the live lamp), as luminance.
fn pair(albedo: f32, roughness: f32, tag: &str) -> Option<[Vec<f32>; 2]> {
    let mut baked = seat(LightMode::Baked, albedo, roughness);
    bake(&mut baked, &format!("stool_{tag}_baked"), true);
    let baked = shot(&baked, &camera(), &format!("stool_{tag}_baked"))?;
    let mut live = seat(LightMode::Mixed, albedo, roughness);
    bake(&mut live, &format!("stool_{tag}_live"), false);
    Some([
        baked,
        shot(&live, &camera(), &format!("stool_{tag}_live")).unwrap(),
    ])
}

/// How much brighter `a` is than `b`, summed over the frame.
fn gain(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(a, b)| a - b).sum()
}

#[test]
fn a_rough_dielectric_under_a_baked_spot_looks_as_under_the_live_one() {
    let Some([baked, live]) = pair(0.03, 0.55, "leather") else {
        eprintln!("[baked-spec] no GPU/software adapter — skipping");
        return;
    };
    let (mean, worst) = (mean_gap(&baked, &live), most(&baked, &live));
    eprintln!("[baked-spec] stool: mean {mean:.2}, at most {worst:.1} apart");
    assert!(
        mean <= 1.5,
        "the seat reads as under the live spot: {mean:.2}"
    );
    assert!(worst <= 8.0, "nowhere far off: {worst:.1}");
    for (albedo, roughness, tag) in [(0.03, 0.9, "rough"), (0.3, 0.55, "pale")] {
        let [b, l] = pair(albedo, roughness, tag).unwrap();
        let (b, l) = (gain(&b, &baked), gain(&l, &live));
        eprintln!("[baked-spec] stool {tag}: baked moves {b:.0}, live {l:.0}");
        assert!(l.abs() > 500.0, "{tag} changes the live seat: {l:.0}");
        let ratio = b / l;
        assert!(
            (1.0 / 1.5..1.5).contains(&ratio),
            "{tag} changes the baked seat as the live one: {ratio:.2}"
        );
    }
}
