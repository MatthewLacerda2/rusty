//! Cascaded sun shadows (#435): a caster 100 units from the world origin — where the
//! old fixed ±30 shadow box never reached — must still darken the ground under it.
//! The same yard is rendered twice, once with the shadow distance cut to nothing, so
//! the only difference between the frames is the shadow. Skips without an adapter.

use glam::{Quat, Vec3};
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{create_entity, default_visual_correction, Primitive};
use rusty::scene::{Camera, Scene};

const FAR_OUT: Vec3 = Vec3::new(100.0, 0.0, -40.0);

/// A ground slab with a floating roof over it at `FAR_OUT`, under a high sun.
fn yard(shadow_distance: f32) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.3);
    for (name, offset, scale) in [
        (
            "Ground",
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(20.0, 1.0, 20.0),
        ),
        ("Roof", Vec3::new(0.0, 3.0, 0.0), Vec3::new(4.0, 0.2, 4.0)),
    ] {
        let id = create_entity(&mut scene, name, Some(Primitive::Box));
        let mut t = scene.world.transform_mut(id).unwrap();
        t.position = FAR_OUT + offset;
        t.scale = scale;
    }
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_visual_correction();
    vc.shadows.distance = shadow_distance;
    // Dimmed and bloom-free, so the lit ground is not clipped white and a shadow reads.
    vc.exposure = -2.0;
    vc.bloom_active = false;
    scene.world.set_visual_correction(volume, Some(vc));
    scene
}

/// Pixels that are clearly darker in `shadowed` than in `unshadowed`.
fn darkened_pixels(shadowed: &std::path::Path, unshadowed: &std::path::Path) -> usize {
    let on = image::open(shadowed).expect("png readable").to_rgb8();
    let off = image::open(unshadowed).expect("png readable").to_rgb8();
    let sum = |p: &image::Rgb<u8>| p.0.iter().map(|&c| c as i32).sum::<i32>();
    on.pixels()
        .zip(off.pixels())
        .filter(|(a, b)| sum(b) - sum(a) > 60)
        .count()
}

#[test]
fn a_caster_far_from_the_origin_still_casts() {
    let cam = Camera::new(FAR_OUT + Vec3::new(0.0, 12.0, 12.0), -90.0, -45.0);
    let dir = crate::temp::dir();
    let shadowed = dir.join(format!("rusty_csm_on_{}.png", std::process::id()));
    let unshadowed = dir.join(format!("rusty_csm_off_{}.png", std::process::id()));
    let on = capture(&yard(100.0), &cam, &shadowed, 96, 96).expect("capture");
    let off = capture(&yard(1.0), &cam, &unshadowed, 96, 96).expect("capture");
    if !on || !off {
        eprintln!("[csm] no GPU/software adapter — skipping visual assertion");
        return;
    }
    let darkened = darkened_pixels(&shadowed, &unshadowed);
    eprintln!("[csm] pixels darkened by the shadow: {darkened}");
    // The roof's shadow covers a few hundred of the 96² pixels; the old fixed box
    // around the origin never reached this far out.
    assert!(
        darkened > 100,
        "the roof 100 units out should shade the ground: {darkened}"
    );
    let _ = std::fs::remove_file(shadowed);
    let _ = std::fs::remove_file(unshadowed);
}
