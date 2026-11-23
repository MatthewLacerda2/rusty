//! A ray hit is fogged over its whole path, as a march hit carries the fog between the camera
//! and it: the same slab reflected from farther away is nearer the fog's colour.

use glam::Vec3;

use super::super::fixture::{draw, paint, place, rgb, tuned, H, MIRROR, W};
use super::{mean, room, shot};
use crate::{RenderView, OFFSCREEN_FORMAT};
use rusty_components::SsrTracing;
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

fn on_screen(cam: &rusty_scene::Camera, p: Vec3) -> (u32, u32) {
    let clip = cam.build_view_projection(W as f32 / H as f32) * p.extend(1.0);
    let ndc = clip.truncate().truncate() / clip.w;
    let x = ((ndc.x * 0.5 + 0.5) * W as f32) as u32;
    let y = ((0.5 - ndc.y * 0.5) * H as f32) as u32;
    (x.min(W - 1), y.min(H - 1))
}

/// A slab's face and the floor point mirroring it, from one eye: level, a march hit;
/// tipped down, the slab off screen, a ray hit.
const SLAB: Vec3 = Vec3::new(0.0, 1.5, -12.0);
const MIRRORED: Vec3 = Vec3::new(0.0, 0.0, -6.0);

fn seen(r: &mut crate::Renderer, pitch: f32, tracing: SsrTracing) -> [i32; 3] {
    let (mut s, _) = tuned(true, MIRROR, MIRROR, |v| v.ssr.tracing = tracing);
    let slab = s.find_entity_by_name("Cube").unwrap();
    place(&mut s, slab, SLAB - Vec3::Z * 0.1, Vec3::new(6.0, 1.6, 0.2));
    paint(&mut s, slab, "cube", ([0.0; 3], 0.0, 1.0));
    s.materials.get_mut("cube").unwrap().emissive = [4.0, 0.0, 0.0];
    for name in ["Cube", "Floor"] {
        let id = s.find_entity_by_name(name).unwrap();
        s.world.set_static(id, true);
    }
    s.fog = FogSettings {
        mode: FogMode::Exponential,
        color: Vec3::new(0.0, 0.2, 1.0),
        density: 0.08,
        start: 0.0,
        height_falloff: 0.5,
        ..Default::default()
    };
    let cam = rusty_scene::Camera::new(Vec3::new(0.0, 1.5, 0.0), -90.0, pitch);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    for _ in 0..2 {
        draw(r, &mut view, &s, &cam);
    }
    let frame = draw(r, &mut view, &s, &cam);
    let c = rgb(&frame, on_screen(&cam, MIRRORED));
    eprintln!("[ssr] mixed fog seam: pitch {pitch}, {tracing:?}: {c:?}");
    c
}

#[test]
fn gpu_mixed_fogs_a_ray_hit_as_a_march_hit_of_the_same_point() {
    let Some(mut r) = super::super::fixture::renderer() else {
        return;
    };
    let march = seen(&mut r, 0.0, SsrTracing::RayMarching);
    let ray = seen(&mut r, -30.0, SsrTracing::Mixed);
    let missed = seen(&mut r, -30.0, SsrTracing::RayMarching);
    assert!(
        ray[0] > missed[0] + 40,
        "the ray hit is the slab: {missed:?} → {ray:?}"
    );
    let gap = (0..3).map(|i| (march[i] - ray[i]).abs()).max().unwrap();
    assert!(gap <= 8, "one fog over both: march {march:?}, ray {ray:?}");
}
