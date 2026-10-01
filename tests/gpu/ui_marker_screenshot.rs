//! World-anchored marker screenshot (#429): a marker lands where the GPU draws its
//! target — the CPU projection agrees with the renderer's — and hides (or clamps to
//! the edge) once the target is behind the camera.

use glam::{Vec2, Vec3, Vec4};
use rusty::components::{CanvasComponent, RectTransformComponent, WorldAnchor};
use rusty::dev::screenshot::capture;
use rusty::scene::{Camera, Scene};

use super::world_ui_scene::{canvas, centroid, dark_scene, dominant, emissive_box, image};

const SIZE: u32 = 128;

/// Render `scene` from the origin looking down -Z; `None` without an adapter.
fn shot(scene: &Scene, name: &str) -> Option<std::path::PathBuf> {
    let path = crate::temp::dir().join(name);
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    capture(scene, &cam, &path, SIZE, SIZE)
        .expect("capture must not error")
        .then_some(path)
}

/// A HUD (one reference unit = one pixel) with an 8×8 green marker on `point`.
fn marker_scene(point: Vec3, anchor: WorldAnchor) -> Scene {
    let mut scene = dark_scene();
    let hud = CanvasComponent {
        reference_resolution: Vec2::splat(SIZE as f32),
        ..Default::default()
    };
    let hud = canvas(&mut scene, hud);
    let rt = RectTransformComponent {
        size_delta: Vec2::splat(8.0),
        world_anchor: Some(WorldAnchor {
            offset: point,
            ..anchor
        }),
        ..Default::default()
    };
    image(&mut scene, hud, rt, Vec4::new(0.0, 1.0, 0.0, 1.0));
    scene
}

#[test]
fn a_marker_lands_on_its_target_and_hides_behind_the_camera() {
    let target = Vec3::new(1.2, -0.6, -6.0);
    // Where the renderer draws a small red box at the target…
    let mut world = dark_scene();
    emissive_box(
        &mut world,
        "Target",
        target,
        Vec3::splat(0.2),
        [1.0, 0.0, 0.0],
    );
    let Some(drawn) = shot(&world, "rusty_marker_target.png") else {
        eprintln!("[marker] no GPU/software adapter — skipping visual assertion");
        return;
    };
    let (box_at, n) = centroid(&drawn, |p| dominant(p, 0));
    assert!(n > 4, "the target box is on screen");
    // …is where the layout puts the marker.
    let path = shot(
        &marker_scene(target, WorldAnchor::default()),
        "rusty_marker.png",
    );
    let (marker_at, n) = centroid(&path.expect("adapter"), |p| dominant(p, 1));
    assert!(n > 30, "the marker draws");
    assert!(
        (marker_at - box_at).length() < 1.5,
        "marker {marker_at} vs target {box_at}"
    );
    assert!(
        box_at.x > 64.0 && box_at.y < 64.0,
        "right of and below centre"
    );

    // Behind the camera: hidden by default…
    let behind = Vec3::new(1.0, 0.0, 6.0);
    let path = shot(
        &marker_scene(behind, WorldAnchor::default()),
        "rusty_marker_behind.png",
    );
    let (_, n) = centroid(&path.expect("adapter"), |p| dominant(p, 1));
    assert_eq!(n, 0, "hidden behind the camera");
    // …or, told to stay, clamped to the screen edge on the target's side (right).
    let stay = WorldAnchor {
        hide_when_behind: false,
        clamp_to_screen_edge: true,
        edge_padding: 8.0,
        ..Default::default()
    };
    let path = shot(&marker_scene(behind, stay), "rusty_marker_clamped.png");
    let (at, n) = centroid(&path.expect("adapter"), |p| dominant(p, 1));
    assert!(n > 30, "the clamped marker draws");
    assert!(
        (at - Vec2::new(120.0, 64.0)).length() < 1.5,
        "clamped at {at}"
    );
}
