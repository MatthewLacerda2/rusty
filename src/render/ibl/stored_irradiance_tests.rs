//! The irradiance a reflection bake stores beside each cube (#1697): the cube's own
//! SH, which reads back the room's colours, plus the direct light of a `Baked` lamp
//! at the probe, which a capture alone never sees.

use glam::Vec3;

use super::reflection_bake_tests::colored_room;
use crate::components::{LightComponent, LightMode, LightType};
use crate::scene::Scene;

const RES: u32 = 32;

/// `scene` with one probe at the origin, baked; its stored irradiance along `n`.
fn bake(mut scene: Scene, name: &str, n: Vec3) -> Option<Vec3> {
    let mut renderer = crate::render::test_gpu::headless_or_skip(RES, RES)?;
    scene.lighting_settings.reflections.resolution = RES;
    scene.reflection_probes.add(Vec3::ZERO, Vec3::splat(5.0));
    let dir = crate::test_temp::dir().join(format!("rusty_1697_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    renderer.bake_reflections(&mut scene, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let stored = scene.reflection_probes.probes[0].irradiance;
    Some(stored.expect("the bake stores the irradiance").eval(n))
}

#[test]
fn gpu_a_bake_stores_the_irradiance_its_cube_gives() {
    // The +X wall glows red, the -X wall green.
    let Some(red) = bake(colored_room(), "room_x", Vec3::X) else {
        return;
    };
    let green = bake(colored_room(), "room_negx", Vec3::NEG_X).unwrap();
    assert!(red.x > red.y && red.x > 1.0, "facing the red wall: {red}");
    assert!(
        green.y > green.x && green.y > 1.0,
        "facing the green wall: {green}"
    );
}

#[test]
fn gpu_a_baked_lamp_beside_the_probe_adds_its_direct_light() {
    let Some(dark) = bake(Scene::new(), "no_lamp", Vec3::Y) else {
        return;
    };
    let mut lit = Scene::new();
    let lamp = lit.add_entity("Lamp".to_string());
    let light = LightComponent {
        light_type: LightType::Point,
        intensity: 10.0,
        range: 10.0,
        mode: LightMode::Baked,
        ..crate::scene::authoring::default_light()
    };
    lit.world.set_light(lamp, Some(light));
    lit.world.transform_mut(lamp).unwrap().position = Vec3::Y * 2.0;
    let lit = bake(lit, "lamp", Vec3::Y).unwrap();
    assert!(
        lit.x > dark.x + 0.5,
        "a Baked lamp above adds its direct light: {lit} vs {dark}"
    );
}
