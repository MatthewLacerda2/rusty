//! SSR on lavapipe (#1526, #1542): a mirror reflects and a rough floor does not, an
//! overlay camera's viewmodel is untouched while the world under it still reflects,
//! and with no skybox the sky an SSR hit replaces is the procedural one.

use glam::Vec3;

use super::fixture::{frame, pixel, renderer, rgb, scene, MIRROR, RED, REFLECTING, ROUGH};
use super::fixture::{H, W};
use crate::components::CameraComponent;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{ClearFlags, Scene};

/// The largest per-channel difference between two frames at `p`.
fn diff(a: &[u8], b: &[u8], p: (u32, u32)) -> i32 {
    let (a, b) = (rgb(a, p), rgb(b, p));
    (0..3).map(|c| (a[c] - b[c]).abs()).max().unwrap()
}

#[test]
fn gpu_a_mirror_reflects_the_cube_and_a_rough_floor_reflects_nothing() {
    let Some(mut r) = renderer() else { return };
    let at = pixel(REFLECTING);
    let on = frame(&mut r, &scene(true, MIRROR, RED).0);
    let off = frame(&mut r, &scene(false, MIRROR, RED).0);
    let (lit, plain) = (rgb(&on, at), rgb(&off, at));
    eprintln!("[ssr] mirror at {at:?}: on {lit:?}, off {plain:?}");
    // Off, the mirror shows the white sky; on, the red cube in its place.
    let redness = |c: [i32; 3]| c[0] - c[2];
    assert!(
        redness(lit) >= redness(plain) + 40,
        "the mirror shows the red cube"
    );

    let rough_on = frame(&mut r, &scene(true, ROUGH, RED).0);
    let rough_off = frame(&mut r, &scene(false, ROUGH, RED).0);
    assert!(rough_on == rough_off, "a rough floor gets no SSR at all");
}

/// The viewmodel's layer: only the overlay camera draws it, and it draws nothing else.
const VIEWMODEL: u8 = 5;

/// `scene` (base camera `cam`) with a `DepthOnly` overlay camera holding a small
/// mirror box at the screen's centre, 3 m ahead of it.
fn with_viewmodel((mut scene, cam): (Scene, u32)) -> Scene {
    scene.world.camera_mut(cam).unwrap().culling_mask = !(1 << VIEWMODEL);
    let overlay = scene.add_entity("ViewmodelCamera".to_string());
    let lens = CameraComponent {
        render_order: 1,
        clear_flags: ClearFlags::DepthOnly,
        culling_mask: 1 << VIEWMODEL,
        ..Default::default()
    };
    scene.world.set_camera(overlay, Some(lens));
    let gun = create_entity(&mut scene, "Gun", Some(Primitive::Box));
    super::fixture::place(&mut scene, gun, Vec3::new(0.0, 0.0, -3.0), Vec3::splat(0.4));
    scene.set_parent(gun, Some(overlay)).unwrap();
    scene.world.set_layer(gun, VIEWMODEL);
    super::fixture::paint(&mut scene, gun, "gun", MIRROR);
    scene
}

#[test]
fn gpu_an_overlay_cameras_pixels_get_no_ssr_and_the_world_still_does() {
    let Some(mut r) = renderer() else { return };
    let on = frame(&mut r, &with_viewmodel(scene(true, MIRROR, RED)));
    let off = frame(&mut r, &with_viewmodel(scene(false, MIRROR, RED)));
    let (cx, cy) = (W / 2, H / 2);
    let mut gun = 0;
    for y in cy - 2..=cy + 2 {
        for x in cx - 2..=cx + 2 {
            gun = gun.max(diff(&on, &off, (x, y)));
        }
    }
    let world = diff(&on, &off, pixel(REFLECTING));
    let (a, b) = (rgb(&on, pixel(REFLECTING)), rgb(&off, pixel(REFLECTING)));
    eprintln!(
        "[ssr] viewmodel: gun differs by {gun}, the reflecting floor by {world}: {a:?} vs {b:?}"
    );
    assert_eq!(gun, 0, "the overlay's mirror gun is untouched");
    assert!(world >= 40, "the floor still reflects under the overlay");
}

#[test]
fn gpu_without_a_skybox_a_hit_replaces_the_procedural_sky() {
    let Some(mut r) = renderer() else { return };
    // A green-tinted procedural sky, and a black cube: where the mirror floor sees
    // the cube, SSR takes away the sky it reflected, so the floor loses green. The
    // missing-texture checker it once read would take away violet: red and blue.
    let black = ([0.0; 3], 0.0, 1.0);
    let green = |on| {
        let (mut s, _) = scene(on, MIRROR, black);
        s.ambient_color = Vec3::new(0.1, 0.9, 0.1);
        s
    };
    let at = pixel(REFLECTING);
    let (on, off) = (
        rgb(&frame(&mut r, &green(true)), at),
        rgb(&frame(&mut r, &green(false)), at),
    );
    let lost = [0, 1, 2].map(|c| off[c] - on[c]);
    eprintln!("[ssr] no skybox: off {off:?}, on {on:?}, lost {lost:?}");
    assert!(lost[1] >= 10, "the hit replaced the reflected sky");
    assert!(
        lost[1] > lost[0].max(lost[2]),
        "what it replaced was green, not violet"
    );
}
