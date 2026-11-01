//! The analytic probe fill: its height stand-in, the sun, and the ambient it projects
//! (#1647), which a probe-lit surface reads back as the forward shader's flat ambient.

use glam::Quat;

use super::*;
use crate::components::{LightComponent, LightType};
use crate::scene::AmbientMode;

/// A default scene with a set ambient term. Built through `Scene::new` rather
/// than functional struct-update: the scene's runtime `id` is private (#355), and
/// going through the constructor is what guarantees every scene gets its own.
fn scene_with_ambient(color: Vec3, intensity: f32) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_color = color;
    scene.ambient_intensity = intensity;
    scene
}

/// The fill of one probe at the origin (where the height stand-in is 1), read back
/// as the forward shader reads a probe: irradiance over π.
fn read_back(scene: &mut Scene, n: Vec3) -> Vec3 {
    scene.probes.probes.clear();
    scene.probes.add_probe(Vec3::ZERO);
    analytic_fill(scene);
    scene.probes.probes[0].sh.eval(n) / std::f32::consts::PI
}

const NORMALS: [Vec3; 4] = [Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::Z];

#[test]
fn fill_is_position_dependent() {
    let mut scene = scene_with_ambient(Vec3::new(0.2, 0.3, 0.4), 1.0);
    scene.probes.add_probe(Vec3::new(0.0, 0.0, 0.0));
    scene.probes.add_probe(Vec3::new(0.0, 4.0, 0.0));
    analytic_fill(&mut scene);
    let low = scene.probes.probes[0].sh.eval(Vec3::Y);
    let high = scene.probes.probes[1].sh.eval(Vec3::Y);
    assert!(
        high.x > low.x,
        "expected height to brighten: {high} vs {low}"
    );
}

#[test]
fn directional_sun_adds_directionality() {
    // No ambient, so up outshining down can only be the sun.
    let mut scene = scene_with_ambient(Vec3::ZERO, 0.0);
    let id = scene.add_entity("Sun".to_string());
    // Point the light's -Z straight down, so the sample direction is +Y.
    if let Some(mut transform) = scene.world.transform_mut(id) {
        transform.rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    }
    scene.world.set_light(
        id,
        Some(LightComponent {
            light_type: LightType::Directional,
            intensity: 5.0,
            range: 0.0,
            inner_cone: 0.0,
            outer_cone: 0.0,
            ..crate::scene::authoring::default_light()
        }),
    );
    let (up, down) = (
        read_back(&mut scene, Vec3::Y),
        read_back(&mut scene, Vec3::NEG_Y),
    );
    assert!(
        up.x > 0.1 && up.x > down.x,
        "sun from above: {up} vs {down}"
    );
}

#[test]
fn a_probe_lit_surface_takes_the_unbaked_ambient_in_both_modes() {
    let mut scene = scene_with_ambient(Vec3::new(0.4, 0.8, 1.2), 1.5);
    let color = scene.drawn_ambient().radiance();
    for n in NORMALS {
        let got = read_back(&mut scene, n);
        assert!(
            got.abs_diff_eq(color.at(n), 5e-3),
            "Color {n}: {got} vs {}",
            color.at(n)
        );
    }
    scene.ambient.mode = AmbientMode::Gradient;
    scene.ambient.sky = Vec3::new(0.1, 0.3, 1.2);
    scene.ambient.ground = Vec3::new(1.2, 0.5, 0.1);
    let gradient = scene.drawn_ambient().radiance();
    for n in NORMALS {
        let (got, want) = (read_back(&mut scene, n), gradient.at_l2(n));
        assert!(got.abs_diff_eq(want, 5e-3), "Gradient {n}: {got} vs {want}");
    }
    // The gradient really reaches the probe: a blue sky above, an orange ground below.
    let (up, down) = (
        read_back(&mut scene, Vec3::Y),
        read_back(&mut scene, Vec3::NEG_Y),
    );
    assert!(up.z > up.x && down.x > down.z, "{up} / {down}");
}

#[test]
fn an_ambient_light_entity_overrides_the_fill() {
    let mut scene = scene_with_ambient(Vec3::ONE, 1.0);
    scene.ambient.mode = AmbientMode::Gradient;
    let id = scene.add_entity("Ambient".into());
    let light = LightComponent {
        light_type: LightType::Ambient,
        color: Vec3::new(1.0, 0.0, 0.0),
        intensity: 2.0,
        ..crate::scene::authoring::default_light()
    };
    scene.world.set_light(id, Some(light));
    let want = AmbientRadiance::Hemisphere(Vec3::new(2.0, 0.0, 0.0));
    for n in NORMALS {
        let got = read_back(&mut scene, n);
        assert!(
            got.abs_diff_eq(want.at(n), 5e-3),
            "{n}: {got} vs {}",
            want.at(n)
        );
    }
}
