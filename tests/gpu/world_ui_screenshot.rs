//! World-space canvas screenshot (#429): a `WorldSpace` canvas is drawn in the
//! scene, depth-tested — an opaque wall in front of half of it hides that half —
//! and a `ScreenSpaceCamera` canvas fills the view in front of the camera.

use glam::{Vec2, Vec3, Vec4};
use rusty::components::{CanvasComponent, CanvasRenderMode};
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;

use super::world_ui_scene::{canvas, dark_scene, dominant, emissive_box, fill, image};

pub(super) const SIZE: u32 = 96;

pub(super) fn shot(scene: &rusty::scene::Scene, name: &str) -> Option<image::RgbImage> {
    let path = std::env::temp_dir().join(name);
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    if !capture(scene, &cam, &path, SIZE, SIZE).expect("capture must not error") {
        eprintln!("[world-ui] no GPU/software adapter — skipping visual assertion");
        return None;
    }
    Some(image::open(&path).expect("png").to_rgb8())
}

#[test]
fn a_wall_occludes_a_world_canvas() {
    let mut scene = dark_scene();
    // A 4×4 m red sign at the origin, facing the camera 5 m away.
    let sign = CanvasComponent {
        render_mode: CanvasRenderMode::WorldSpace,
        reference_resolution: Vec2::splat(400.0),
        ..Default::default()
    };
    let sign = canvas(&mut scene, sign);
    image(&mut scene, sign, fill(), Vec4::new(1.0, 0.0, 0.0, 1.0));
    // A blue wall 2 m in front of the sign, covering the left half of the view.
    let wall = Vec3::new(4.0, 8.0, 0.2);
    emissive_box(
        &mut scene,
        "Wall",
        Vec3::new(-2.0, 0.0, 2.0),
        wall,
        [0.0, 0.0, 1.0],
    );
    let Some(img) = shot(&scene, "rusty_world_ui_wall.png") else {
        return;
    };
    let px = |x: u32| img.get_pixel(x, SIZE / 2).0;
    assert!(
        dominant(px(SIZE * 3 / 4), 0),
        "the sign shows: {:?}",
        px(SIZE * 3 / 4)
    );
    assert!(
        dominant(px(SIZE / 4), 2),
        "the wall hides it: {:?}",
        px(SIZE / 4)
    );
    // Behind the camera it draws nothing.
    let mut behind = dark_scene();
    let back = CanvasComponent {
        render_mode: CanvasRenderMode::WorldSpace,
        ..Default::default()
    };
    let back = canvas(&mut behind, back);
    behind.world.transform_mut(back).expect("t").position = Vec3::new(0.0, 0.0, 10.0);
    image(&mut behind, back, fill(), Vec4::new(1.0, 0.0, 0.0, 1.0));
    let Some(img) = shot(&behind, "rusty_world_ui_behind.png") else {
        return;
    };
    assert!(img.pixels().all(|p| !dominant(p.0, 0)), "nothing red");
}

/// A dark scene with a `ScreenSpaceCamera` canvas 1 m out whose middle quarter
/// is green, and a blue wall filling the view 5 m out, behind the plane.
fn visor_scene() -> rusty::scene::Scene {
    let mut scene = dark_scene();
    let visor = CanvasComponent {
        render_mode: CanvasRenderMode::ScreenSpaceCamera,
        reference_resolution: Vec2::splat(SIZE as f32),
        plane_distance: 1.0,
        ..Default::default()
    };
    let visor = canvas(&mut scene, visor);
    let mut rt = fill();
    rt.anchor_min = Vec2::splat(0.25);
    rt.anchor_max = Vec2::splat(0.75);
    image(&mut scene, visor, rt, Vec4::new(0.0, 1.0, 0.0, 1.0));
    let big = Vec3::new(20.0, 20.0, 0.2);
    emissive_box(&mut scene, "Far", Vec3::ZERO, big, [0.0, 0.0, 1.0]);
    scene
}

#[test]
fn a_camera_canvas_fills_the_view_in_front_of_the_world() {
    let mut scene = visor_scene();
    let Some(img) = shot(&scene, "rusty_world_ui_visor.png") else {
        return;
    };
    // Pixel rows run top-down; `at` takes UI pixels (bottom-left, y-up).
    let at = |y: u32| img.get_pixel(SIZE / 2, SIZE - 1 - y).0;
    assert!(dominant(at(SIZE / 2), 1), "centre: {:?}", at(SIZE / 2));
    assert!(dominant(at(30), 1), "inside the quarter: {:?}", at(30));
    assert!(dominant(at(20), 2), "outside it, the wall: {:?}", at(20));
    // A wall closer than the plane covers it.
    let near = Vec3::new(20.0, 20.0, 0.05);
    emissive_box(
        &mut scene,
        "Near",
        Vec3::new(0.0, 0.0, 4.5),
        near,
        [1.0, 0.0, 0.0],
    );
    let Some(img) = shot(&scene, "rusty_world_ui_visor_covered.png") else {
        return;
    };
    let centre = img.get_pixel(SIZE / 2, SIZE / 2).0;
    assert!(dominant(centre, 0), "covered: {centre:?}");
}
