//! Scene fog is consistent across passes (#437) — the issue's key point. Behind
//! fog that fully swallows a wall, a particle (alpha or additive) in front of that
//! wall must vanish into the fog colour too, never glow through it. Each case is
//! also rendered without fog, to prove the effect really covers the centre pixel.
//! Decals are folded into the surface's own material before it is lit and fogged
//! (#638), so they share the wall's fog by construction and need no case here.

use glam::Vec3;
use rusty::components::particle::{Particle, ParticleBlend, ParticleEmitterComponent};
use rusty::core::curve::ColorRange;
use rusty::dev::capture::CaptureHost;
use rusty::scene::{FogMode, Scene};

use super::fog_scene::{assert_close, centre, wall_scene};

const DISTANCE: f32 = 20.0;

/// A plain white sprite, so the effect's colour is its tint alone (the default
/// checker would leave the centre pixel on whichever square it lands).
fn white_sprite() -> Option<String> {
    let path = crate::temp::dir().join("rusty_fog_white.png");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([255; 4]))
        .save(&path)
        .expect("write sprite");
    Some(path.to_string_lossy().into_owned())
}

/// The wall with one particle in front of it at the centre; `fog` fully fogs everything past 1 m.
fn scene(effect: &str, fog: bool) -> Scene {
    let mut scene = wall_scene(DISTANCE, [0.0, 0.0, 0.0]);
    let at = Vec3::new(0.0, 0.0, 5.0 - DISTANCE + 0.5);
    let id = scene.add_entity("Emitter".to_string());
    let mut emitter = ParticleEmitterComponent {
        blend: if effect == "additive" {
            ParticleBlend::Additive
        } else {
            ParticleBlend::Alpha
        },
        color: ColorRange::constant([1.0, 0.2, 0.2, 1.0]),
        texture: white_sprite(),
        ..Default::default()
    };
    emitter
        .runtime
        .particles
        .push(Particle::at(at, 8.0, [1.0, 0.2, 0.2, 1.0], 10.0));
    scene.world.set_particles(id, Some(emitter));
    if fog {
        scene.fog.mode = FogMode::Linear;
        scene.fog.color = Vec3::new(0.2, 0.4, 0.9);
        scene.fog.start = 0.0;
        scene.fog.end = 1.0;
    }
    scene
}

#[test]
fn particles_fade_into_fog_like_the_wall_behind_them() {
    let mut host = CaptureHost::new();
    let mut bare = wall_scene(DISTANCE, [0.0, 0.0, 0.0]);
    bare.fog = scene("alpha", true).fog;
    let Some(fog_only) = centre(&mut host, &bare, "fog_only") else {
        return;
    };
    for effect in ["alpha", "additive"] {
        let (Some(clear), Some(fogged)) = (
            centre(&mut host, &scene(effect, false), &format!("{effect}_clear")),
            centre(&mut host, &scene(effect, true), &format!("{effect}_fogged")),
        ) else {
            return;
        };
        eprintln!("[fog] {effect}: clear={clear:?} fogged={fogged:?} fog={fog_only:?}");
        assert!(
            clear[0] > 60,
            "{effect} must cover the centre pixel ({clear:?})"
        );
        let want = fog_only.map(i32::from);
        assert_close(fogged, want, 2, effect);
    }
}
