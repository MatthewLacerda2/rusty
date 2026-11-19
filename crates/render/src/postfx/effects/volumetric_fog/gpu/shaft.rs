//! A shadowed spot through fog (#1703): its cone lights the air, and the part of the
//! cone a plank shadows stays dark: a shaft with a dark side.

use glam::Vec3;

use super::fixture::{block, fog, glow, renderer, room, settled};
use rusty_components::{LightComponent, LightMode, LightType};
use rusty_scene::create::default_light;

#[test]
fn gpu_a_shadowed_spot_casts_a_shaft_with_a_dark_side() {
    let Some(mut r) = renderer() else { return };
    let mut scene = room(fog());
    let spot = scene.add_entity("Spot".to_string());
    {
        let mut t = scene.world.transform_mut(spot).unwrap();
        t.position = Vec3::new(0.0, 4.0, -5.0);
        // Looking straight down: -Z turned onto -Y.
        t.rotation = glam::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    }
    let light = LightComponent {
        light_type: LightType::Spotlight,
        intensity: 60.0,
        range: 8.0,
        inner_cone: 30.0,
        outer_cone: 40.0,
        cast_shadows: true,
        mode: LightMode::Realtime,
        ..default_light()
    };
    scene.world.set_light(spot, Some(light));
    // A plank under the left half of the cone.
    block(
        &mut scene,
        "plank",
        (Vec3::new(-1.0, 2.5, -5.0), Vec3::new(2.0, 0.1, 4.0)),
    );
    // PROBE (#2385): per-frame values and counters, removed before ready.
    let mut v = super::fixture::view(&r);
    let mut frame = Vec::new();
    for f in 1..=4 {
        frame = super::fixture::draw(&mut r, &mut v, &scene);
        let c = r.frame_counters;
        eprintln!(
            "PROBE f{f} lit {} dark {} floor {} | {c:?}",
            glow(&frame, Vec3::new(0.8, 1.2, -5.0)),
            glow(&frame, Vec3::new(-0.8, 1.2, -5.0)),
            glow(&frame, Vec3::new(0.8, 0.0, -5.0)),
        );
    }
    let _ = settled;
    let lit = glow(&frame, Vec3::new(0.8, 1.2, -5.0));
    let dark = glow(&frame, Vec3::new(-0.8, 1.2, -5.0));
    eprintln!("[volumetric] shaft: {lit} lit side, {dark} shadowed side");
    assert!(lit > 0.3, "the lit cone glows: {lit}");
    assert!(
        lit > 2.0 * dark,
        "the plank's shadow is dark: {lit} vs {dark}"
    );
}
