use super::fixture::{frame, pixel, renderer, rgb, tuned, MIRROR, RED, REFLECTING};
use crate::Renderer;
use rusty_components::RenderMode;
use rusty_scene::Scene;

fn pool(on: bool, receive: bool, dz: f32) -> Scene {
    let (mut scene, _) = tuned(on, MIRROR, RED, |_| {});
    let floor = scene.materials.get_mut("floor").unwrap();
    (floor.render_mode, floor.alpha, floor.receive_ssr) =
        (RenderMode::Transparent, 0.5, Some(receive));
    let cube = scene.find_entity_by_name("Cube").unwrap();
    scene.world.transform_mut(cube).unwrap().position.z += dz;
    scene
}

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

fn busy_normal_map() -> String {
    let mut state = 0x2545_f491u32;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        64 + (state >> 24) as u8 / 2
    };
    let img = image::RgbaImage::from_fn(128, 128, |_, _| {
        let (x, y) = (next(), next());
        let (nx, ny) = (x as f32 / 127.5 - 1.0, y as f32 / 127.5 - 1.0);
        let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
        image::Rgba([x, y, (nz * 127.5 + 127.5) as u8, 255])
    });
    let path = rusty_test_support::temp::dir().join("rusty_1702_ssr_bumps.png");
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn gpu_a_busy_normal_mapped_transparent_floor_keeps_its_ssr() {
    let Some(mut r) = renderer() else { return };
    let at = pixel(REFLECTING);
    let busy = |receive: bool| {
        let mut scene = pool(true, receive, 0.0);
        let floor = scene.materials.get_mut("floor").unwrap();
        (floor.normal_map, floor.tiling) = (Some(busy_normal_map().into()), [40.0, 60.0]);
        scene
    };
    let (traced, _) = shot(&mut r, &busy(true), at);
    let (plain, _) = shot(&mut r, &busy(false), at);
    eprintln!("[ssr] busy transparent floor at {at:?}: receive {traced:?}, not {plain:?}");
    assert!(
        redness(traced) >= redness(plain) + 30,
        "the busy floor still shows the red cube"
    );
}
