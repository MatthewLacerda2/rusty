//! Transparent SSR on lavapipe (#1570): a glossy Transparent floor with
//! `receive_ssr` reflects the cube beside it, wherever the cube moves, and without
//! it (or with SSR off) shows only the sky it reflected before, and makes no copy.

use super::fixture::{frame, pixel, renderer, rgb, tuned, MIRROR, RED, REFLECTING};
use crate::components::RenderMode;
use crate::render::Renderer;
use crate::scene::Scene;

/// [`tuned`]'s scene, SSR `on`, its floor a half-clear glossy mirror that
/// receives SSR when `receive`, and its cube moved `dz` along Z.
fn pool(on: bool, receive: bool, dz: f32) -> Scene {
    let (mut scene, _) = tuned(on, MIRROR, RED, |_| {});
    let floor = scene.materials.get_mut("floor").unwrap();
    (floor.render_mode, floor.alpha, floor.receive_ssr) = (RenderMode::Transparent, 0.5, receive);
    let cube = scene.find_entity_by_name("Cube").unwrap();
    scene.world.transform_mut(cube).unwrap().position.z += dz;
    scene
}

/// The frame at `at`, and how many opaque copies it made.
fn shot(r: &mut Renderer, scene: &Scene, at: (u32, u32)) -> ([i32; 3], u32) {
    let image = frame(r, scene);
    (rgb(&image, at), r.frame_counters.opaque_copies)
}

fn redness(c: [i32; 3]) -> i32 {
    c[0] - c[2]
}

#[test]
fn gpu_a_transparent_floor_with_receive_ssr_reflects_the_cube_where_it_moved() {
    let Some(mut r) = renderer() else { return };
    let at = pixel(REFLECTING);
    let (traced, copies) = shot(&mut r, &pool(true, true, 0.0), at);
    let (plain, none) = shot(&mut r, &pool(true, false, 0.0), at);
    eprintln!("[ssr] transparent floor at {at:?}: receive {traced:?}, not {plain:?}");
    assert!(
        redness(traced) >= redness(plain) + 30,
        "the floor shows the red cube"
    );
    assert_eq!((copies, none), (1, 0), "only a tracing floor makes a copy");
    // Moved 4 m back, the cube leaves the ray: the floor shows the sky again.
    let (moved, _) = shot(&mut r, &pool(true, true, -4.0), at);
    eprintln!("[ssr] cube moved back: {moved:?}");
    assert!(
        redness(moved) + 30 <= redness(traced),
        "the reflection follows the cube"
    );
}

#[test]
fn gpu_receive_ssr_traces_nothing_where_ssr_is_off() {
    let Some(mut r) = renderer() else { return };
    let off = frame(&mut r, &pool(false, true, 0.0));
    assert_eq!(r.frame_counters.opaque_copies, 0, "no copy without SSR");
    let plain = frame(&mut r, &pool(false, false, 0.0));
    assert!(off == plain, "SSR off: receive_ssr changes no pixel");
}
