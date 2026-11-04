//! What reads the fogged colour after the forward pass (#1703): SSIL's bounce off a
//! fogged surface is dimmed by the same fog the surface got, so the fog never
//! bounces light it took. Black fog (albedo 0) only takes light, which keeps the
//! measure clean. SSR's record reads the same factor (`transparent::surface_fog`).

use glam::Vec3;

use super::fixture::{channel, fog, glow, lamp, renderer, room, set_fog, settled, tinted};
use crate::components::{LightMode, VolumetricFog};
use crate::scene::Scene;

/// The floor point beside the lamp's pool, half a metre in front of the red wall.
const FLOOR: Vec3 = Vec3::new(1.5, 0.0, -5.5);

fn set_ssil(scene: &mut Scene, active: bool) {
    for id in scene.world.ids_with_post_processing() {
        let ssil = &mut scene.world.post_processing_mut(id).unwrap().ssil;
        (ssil.active, ssil.intensity) = (active, 4.0);
    }
}

#[test]
fn gpu_ssil_bounces_no_light_the_fog_took() {
    let Some(mut r) = renderer() else { return };
    let mut scene = room(VolumetricFog::default());
    let floor = (Vec3::new(0.0, 0.0, -6.0), Vec3::new(20.0, 0.1, 20.0));
    tinted(&mut scene, "white", floor, [0.9; 3]);
    let wall = (Vec3::new(0.0, 1.5, -6.0), Vec3::new(6.0, 3.0, 0.2));
    tinted(&mut scene, "red", wall, [1.0, 0.05, 0.05]);
    lamp(
        &mut scene,
        Vec3::new(0.0, 1.0, -5.3),
        6.0,
        LightMode::Realtime,
    );
    let mut read = |scene: &mut Scene, ssil: bool, black: Option<VolumetricFog>| {
        set_ssil(scene, ssil);
        set_fog(scene, black.unwrap_or_default());
        let frame = settled(&mut r, scene, 6);
        (channel(&frame, FLOOR, 0), glow(&frame, FLOOR))
    };
    let black = VolumetricFog {
        albedo: [0.0; 3],
        mean_free_path: 8.0,
        ..fog()
    };
    let (plain_off, plain) = read(&mut scene, false, None);
    let (plain_on, _) = read(&mut scene, true, None);
    let (fogged_off, fogged) = read(&mut scene, false, Some(black));
    let (fogged_on, _) = read(&mut scene, true, Some(black));
    let through = fogged / plain;
    let (bounce, fogged_bounce) = (plain_on - plain_off, fogged_on - fogged_off);
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
