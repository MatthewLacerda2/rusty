//! A shadowed spot through fog (#1703): its cone lights the air, and the part of the
//! cone a plank shadows stays dark: a shaft with a dark side.

use glam::Vec3;

use super::fixture::{block, fog, glow, renderer, room, settled};
use crate::components::{LightComponent, LightMode, LightType};
use crate::scene::authoring::default_light;

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
    let frame = settled(&mut r, &scene, 4);
    let lit = glow(&frame, Vec3::new(0.8, 1.2, -5.0));
    let dark = glow(&frame, Vec3::new(-0.8, 1.2, -5.0));
    eprintln!("[volumetric] shaft: {lit} lit side, {dark} shadowed side");
    assert!(lit > 0.3, "the lit cone glows: {lit}");
    assert!(
        lit > 2.0 * dark,
        "the plank's shadow is dark: {lit} vs {dark}"
    );
}
