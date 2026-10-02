//! Soft, lit and mesh particles (#440): a soft sprite touching a wall fades into
//! it, a lit sprite is dark without light and brightens beside a point light, and
//! a mesh particle draws its mesh with its material — counted as a particle.

use glam::Vec3;
use rusty::components::ParticleRenderMode;
use rusty::components::{LightComponent, LightType, MaterialAsset, ParticleRender};
use rusty::dev::capture::CaptureHost;

use super::particle_scene::{is_red, one, pixel, scene_with};

#[test]
fn a_soft_particle_fades_into_the_wall_it_touches() {
    let mut host = CaptureHost::new();
    // The wall's front face is 20 m out (z = −15); the sprite sits 5 cm before it.
    let at = Vec3::new(0.0, 0.0, -14.9);
    let (hard, _) = scene_with(ParticleRender::default(), one(at, 6.0));
    let soft_render = ParticleRender {
        soft_distance: 2.0,
        ..Default::default()
    };
    let (soft, _) = scene_with(soft_render, one(at, 6.0));
    let (Some(h), Some(s)) = (
        pixel(&mut host, &hard, "soft_off", 32, 32),
        pixel(&mut host, &soft, "soft_on", 32, 32),
    ) else {
        return;
    };
    assert!(is_red(h), "a hard sprite covers the wall: {h:?}");
    assert!(
        s[0] < h[0] / 2,
        "the soft one nearly vanishes: {s:?} vs {h:?}"
    );
}

#[test]
fn a_lit_particle_is_dark_unlit_and_brightens_by_a_light() {
    let mut host = CaptureHost::new();
    let lit = ParticleRender {
        lit: true,
        ..Default::default()
    };
    let (unlit_scene, _) = scene_with(ParticleRender::default(), one(Vec3::ZERO, 2.0));
    let (dark, _) = scene_with(lit.clone(), one(Vec3::ZERO, 2.0));
    let (mut bright, _) = scene_with(lit, one(Vec3::ZERO, 2.0));
    let lamp = bright.add_entity("Lamp".to_string());
    bright.world.transform_mut(lamp).unwrap().position = Vec3::new(0.0, 0.0, 1.0);
    bright.world.set_light(
        lamp,
        Some(LightComponent {
            light_type: LightType::Point,
            color: Vec3::ONE,
            intensity: 20.0,
            range: 10.0,
            inner_cone: 0.0,
            outer_cone: 0.0,
            cast_shadows: false,
        }),
    );
    let (Some(u), Some(d), Some(b)) = (
        pixel(&mut host, &unlit_scene, "lit_off", 32, 32),
        pixel(&mut host, &dark, "lit_dark", 32, 32),
        pixel(&mut host, &bright, "lit_lamp", 32, 32),
    ) else {
        return;
    };
    assert!(is_red(u), "unlit shows its own colour: {u:?}");
    assert!(d[0] < 10, "no ambient, no lights: black ({d:?})");
    assert!(b[0] > d[0] + 80, "a point light brightens it: {b:?}");
}

#[test]
fn a_mesh_particle_draws_its_mesh_and_counts_as_a_particle() {
    let mut host = CaptureHost::new();
    let render = ParticleRender {
        mode: ParticleRenderMode::Mesh,
        // Not "Box": the wall fixture registers its own geometry under that name.
        mesh: Some("Sphere".to_string()),
        material: Some("glow".to_string()),
        ..Default::default()
    };
    let (mut scene, _) = scene_with(render, one(Vec3::ZERO, 1.0));
    scene.materials.insert(
        "glow".to_string(),
        MaterialAsset {
            base_color: [0.0; 3],
            emissive: [1.0, 0.1, 0.1],
            // Fully rough: no sky reflection (#718), so the pixel is the emissive alone.
            roughness: 1.0,
            ..MaterialAsset::default()
        },
    );
    let Some(px) = pixel(&mut host, &scene, "mesh", 32, 32) else {
        return;
    };
    assert!(is_red(px), "the sphere shows its emissive material: {px:?}");
    let (renderer, _) = host.frame(64, 64).expect("adapter exists");
    let counters = renderer.frame_counters;
    assert_eq!(counters.particles_drawn, 1, "{counters:?}");
    // The wall is the only entity; the sphere is a particle, not an entity.
    assert_eq!(counters.visible_entities, 1, "{counters:?}");
}
