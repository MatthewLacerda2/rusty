use glam::{Quat, Vec2, Vec3};

use super::*;
use crate::components::{CanvasComponent, CanvasRenderMode};
use crate::scene::{Camera, Scene};

const SCREEN: Vec2 = Vec2::new(1600.0, 900.0);

fn canvas(mode: CanvasRenderMode) -> CanvasComponent {
    CanvasComponent {
        render_mode: mode,
        reference_resolution: Vec2::new(400.0, 200.0),
        ..Default::default()
    }
}

/// A camera at `pos` looking down -Z.
fn camera(pos: Vec3) -> Camera {
    let mut cam = Camera::new(pos, -90.0, 0.0);
    cam.fov = 60.0;
    cam
}

fn close3(a: Vec3, b: Vec3) -> bool {
    (a - b).abs().max_element() < 1e-3
}

#[test]
fn a_world_canvas_is_centred_on_its_transform_in_metres() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Sign".to_string());
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = Vec3::new(1.0, 2.0, 3.0);
        t.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    }
    let c = canvas(CanvasRenderMode::WorldSpace);
    let CanvasSpace::World(m) = canvas_space(&scene.world, id, &c, &UiView::screen(SCREEN)) else {
        panic!("a world canvas lives in the world");
    };
    // 400×200 units at 100 units/m: 4×2 m, centred on the Transform.
    assert!(close3(
        m.transform_point3(Vec3::new(200.0, 100.0, 0.0)),
        Vec3::new(1.0, 2.0, 3.0)
    ));
    // Its +X (rotated 90° about Y) runs along world -Z.
    let right = m.transform_point3(Vec3::new(400.0, 100.0, 0.0));
    assert!(close3(right, Vec3::new(1.0, 2.0, 1.0)));
}

#[test]
fn the_camera_ray_hits_a_world_canvas_where_it_is_drawn() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Sign".to_string());
    let c = canvas(CanvasRenderMode::WorldSpace);
    let view = UiView::with_camera(SCREEN, camera(Vec3::new(0.5, 0.25, 5.0)));
    let CanvasSpace::World(m) = canvas_space(&scene.world, id, &c, &view) else {
        panic!("world");
    };
    // The centre ray from (0.5, 0.25, 5) looking down -Z crosses the plane z = 0 at
    // (0.5, 0.25): 50 and 25 units right of / above the canvas centre.
    let pointer = view.pointer(None);
    let (p, dist) = ray_to_canvas(m, pointer.ray.expect("a camera ray")).expect("a hit");
    assert!((p - Vec2::new(250.0, 125.0)).length() < 1e-2, "{p}");
    assert!((dist - 5.0).abs() < 0.2);
    // Round trip: that canvas point projects back to the screen centre.
    let px = canvas_to_screen(CanvasSpace::World(m), 1.0, p, &view).expect("in front");
    assert!((px - SCREEN * 0.5).length() < 0.5);
    // A ray pointing away from the plane misses.
    assert_eq!(ray_to_canvas(m, (Vec3::new(0.0, 0.0, 5.0), Vec3::Z)), None);
}

#[test]
fn a_camera_canvas_fills_the_view_at_zero_tilt() {
    let scene = Scene::new();
    let c = CanvasComponent {
        render_mode: CanvasRenderMode::ScreenSpaceCamera,
        plane_distance: 2.0,
        ..Default::default()
    };
    let view = UiView::with_camera(SCREEN, camera(Vec3::ZERO));
    let space = canvas_space(&scene.world, 0, &c, &view);
    let size = c.size(SCREEN);
    let scale = c.scale_factor(SCREEN);
    for corner in [Vec2::ZERO, size, Vec2::new(size.x, 0.0), size * 0.25] {
        let px = canvas_to_screen(space, scale, corner, &view).expect("in front");
        assert!((px - corner * scale).length() < 0.5, "{corner} → {px}");
    }
    let CanvasSpace::World(m) = space else {
        panic!("world")
    };
    assert!(close3(
        m.transform_point3((size * 0.5).extend(0.0)),
        Vec3::new(0.0, 0.0, -2.0)
    ));
    // Without a camera it is the overlay.
    assert_eq!(
        canvas_space(&scene.world, 0, &c, &UiView::screen(SCREEN)),
        CanvasSpace::Screen
    );
}

#[test]
fn tilt_leans_the_top_away() {
    let scene = Scene::new();
    let c = CanvasComponent {
        render_mode: CanvasRenderMode::ScreenSpaceCamera,
        tilt: Vec2::new(30.0, 0.0),
        ..Default::default()
    };
    let view = UiView::with_camera(SCREEN, camera(Vec3::ZERO));
    let CanvasSpace::World(m) = canvas_space(&scene.world, 0, &c, &view) else {
        panic!("world")
    };
    let size = c.size(SCREEN);
    let top = m.transform_point3(Vec3::new(size.x * 0.5, size.y, 0.0));
    let bottom = m.transform_point3(Vec3::new(size.x * 0.5, 0.0, 0.0));
    assert!(top.z < bottom.z, "the top is farther: {top} vs {bottom}");
}
