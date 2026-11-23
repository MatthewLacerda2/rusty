//! A ray hit is fogged over its whole path, as a march hit carries the fog between the camera
//! and it: the same slab reflected from farther away is nearer the fog's colour.

use glam::Vec3;

use super::super::fixture::{place, MIRROR};
use super::{mean, room, shot};
use rusty_scene::{FogMode, FogSettings};

/// Where the floor under the screen's middle reflects to, `k` times `ABOVE`'s path.
fn along(k: f32) -> Vec3 {
    let floor = Vec3::new(0.0, 0.0, -0.55);
    floor + (super::ABOVE - floor) * k
}

fn reflected(r: &mut crate::Renderer, k: f32, fogged: bool) -> [i32; 3] {
    let mut s = room(MIRROR, true, |_| {});
    let cube = s.find_entity_by_name("Cube").unwrap();
    place(&mut s, cube, along(k), Vec3::new(4.0 * k, 1.0, 4.0 * k));
    if fogged {
        s.fog = FogSettings {
            mode: FogMode::Exponential,
            color: Vec3::new(0.0, 0.2, 1.0),
            density: 0.04,
            start: 0.0,
            height_falloff: 0.0,
            ..Default::default()
        };
    }
    let c = mean(&shot(r, &s));
    eprintln!("[ssr] mixed fog: {k}× away, fogged {fogged}: {c:?}");
    c
}

#[test]
fn gpu_mixed_fogs_a_ray_hit_more_the_farther_it_is() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let clear = reflected(&mut r, 3.0, false);
    let near = reflected(&mut r, 1.0, true);
    let far = reflected(&mut r, 3.0, true);
    // The fog's blue over the slab's red, as the display shows it (red stays saturated).
    let blue = |c: [i32; 3]| c[2] - c[1];
    assert!(
        blue(near) > blue(clear),
        "near, fogged: {clear:?} → {near:?}"
    );
    assert!(
        blue(far) > blue(near) + 20,
        "farther, nearer the fog's blue: near {near:?}, far {far:?}"
    );
}
