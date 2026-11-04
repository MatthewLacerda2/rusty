//! What reads the fogged colour after the forward pass (#1703): SSIL's bounce off a
//! fogged surface, and SSR's reflection on one, are dimmed by the same fog the
//! surface got (`transparent::surface_fog`), so neither adds light the fog took.
//! Black fog (albedo 0) only takes light, which keeps the measure clean.

use glam::Vec3;

use super::fixture::{channel, fog, glow, lamp, renderer, room, set_fog, settled, tinted};
use crate::components::{LightMode, MaterialAsset, VolumetricFog};
use crate::scene::Scene;

/// The floor point beside the lamp's pool, half a metre in front of the red wall.
const FLOOR: Vec3 = Vec3::new(1.5, 0.0, -5.5);

/// Black fog thick enough to take a clear share of the floor's light.
fn black() -> VolumetricFog {
    VolumetricFog {
        albedo: [0.0; 3],
        mean_free_path: 8.0,
        ..fog()
    }
}

/// The floor point's red and summed light, effect `on` or not, under `fog`
/// (`None`: off): `set` turns the effect.
fn read(
    r: &mut crate::render::Renderer,
    scene: &mut Scene,
    (set, on): (fn(&mut Scene, bool), bool),
    fog: Option<VolumetricFog>,
) -> (f32, f32) {
    set(scene, on);
    set_fog(scene, fog.unwrap_or_default());
    let frame = settled(r, scene, 6);
    (channel(&frame, FLOOR, 0), glow(&frame, FLOOR))
}

/// The red wall behind `FLOOR` and the lamp that lights it.
fn wall_and_lamp(scene: &mut Scene) {
    let wall = (Vec3::new(0.0, 1.5, -6.0), Vec3::new(6.0, 3.0, 0.2));
    tinted(scene, "red", wall, [1.0, 0.05, 0.05]);
    lamp(scene, Vec3::new(0.0, 1.0, -5.3), 6.0, LightMode::Realtime);
}

/// The effect's gain at the floor clear and fogged, and how much of the floor's
/// light the fog lets through, for `set`.
fn gains(scene: &mut Scene, set: fn(&mut Scene, bool)) -> Option<(f32, f32, f32)> {
    let mut r = renderer()?;
    let (plain_off, plain) = read(&mut r, scene, (set, false), None);
    let (plain_on, _) = read(&mut r, scene, (set, true), None);
    let (fogged_off, fogged) = read(&mut r, scene, (set, false), Some(black()));
    let (fogged_on, _) = read(&mut r, scene, (set, true), Some(black()));
    Some((plain_on - plain_off, fogged_on - fogged_off, fogged / plain))
}

fn set_ssr(scene: &mut Scene, active: bool) {
    for id in scene.world.ids_with_post_processing() {
        scene.world.post_processing_mut(id).unwrap().ssr.active = active;
    }
}

fn set_ssil(scene: &mut Scene, active: bool) {
    for id in scene.world.ids_with_post_processing() {
        let ssil = &mut scene.world.post_processing_mut(id).unwrap().ssil;
        (ssil.active, ssil.intensity) = (active, 4.0);
    }
}

#[test]
fn gpu_ssil_bounces_no_light_the_fog_took() {
    let mut scene = room(VolumetricFog::default());
    let floor = (Vec3::new(0.0, 0.0, -6.0), Vec3::new(20.0, 0.1, 20.0));
    tinted(&mut scene, "white", floor, [0.9; 3]);
    wall_and_lamp(&mut scene);
    let Some((bounce, fogged_bounce, through)) = gains(&mut scene, set_ssil) else {
        return;
    };
    eprintln!("[volumetric] bounce {bounce} clear, {fogged_bounce} fogged, through {through}");
    assert!(
        bounce > 0.0 && through < 0.9,
        "a bounce, and fog that takes light"
    );
    assert!(
        fogged_bounce <= bounce * through * 1.05 + 1e-3,
        "the fog dims the bounce at least as it dims the surface"
    );
}

#[test]
fn gpu_ssr_reflects_no_light_the_fog_took() {
    let mut scene = room(VolumetricFog::default());
    let floor = (Vec3::new(0.0, 0.0, -6.0), Vec3::new(20.0, 0.1, 20.0));
    tinted(&mut scene, "mirror", floor, [0.9; 3]);
    let mirror = MaterialAsset {
        base_color: [0.9; 3],
        metallic: 1.0,
        roughness: 0.05,
        ..MaterialAsset::default()
    };
    scene.materials.insert("mirror".into(), mirror);
    wall_and_lamp(&mut scene);
    let Some((reflected, fogged_reflected, through)) = gains(&mut scene, set_ssr) else {
        return;
    };
    eprintln!("[volumetric] SSR {reflected} clear, {fogged_reflected} fogged, through {through}");
    assert!(
        reflected > 0.0 && through < 0.9,
        "a reflection, and fog that takes light"
    );
    assert!(
        fogged_reflected <= reflected * through * 1.05 + 1e-3,
        "the fog dims the reflection at least as it dims the surface"
    );
}
