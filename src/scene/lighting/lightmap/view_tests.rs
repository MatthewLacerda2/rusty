//! Prioritize View (#1970): a progressive bake traces the batches the camera sees
//! first, and only that order moves.

use std::sync::{Arc, Mutex};

use glam::Vec3;

use super::bake::progressive::progressive_on;
use super::progressive_tests::camera;
use super::tests::{floor, quad, settings, wall};
use super::*;

/// Hears which chart each batch starts on, in order.
#[derive(Default)]
struct Order(Mutex<Vec<usize>>);

impl BakeObserver for Order {
    fn meshes(&self, _: &[(u32, u64)]) {}
    fn batch_started(&self, chart: usize) {
        self.0.lock().unwrap().push(chart);
    }
    fn batch_finished(&self, _: usize, _: usize) {}
}

#[test]
fn prioritize_view_traces_what_the_camera_sees_first() {
    // A narrow view of the wall's face from in front of it: the floor is below it.
    let view = camera(Vec3::new(-3.0, 2.0, 0.0), Vec3::new(2.0, 2.0, 0.0), 0.3);
    let scene = BakeScene {
        meshes: vec![floor(), wall()],
        ..Default::default()
    };
    let s = BakeSettings {
        samples: 2,
        samples_per_pass: 1,
        ..settings()
    };
    let order = |view| {
        let heard = Arc::new(Order::default());
        let progress = BakeProgress::observed(heard.clone());
        progressive_on(&scene, &s, &progress, (view, 1), &mut |_, _| {}).expect("finished");
        let order = heard.0.lock().unwrap().clone();
        order
    };
    // One batch a chart each pass: floor (0) then wall (1), unless the wall is seen.
    assert_eq!(order(None), [0, 1, 0, 1]);
    assert_eq!(order(Some(view)), [1, 0, 1, 0]);
    // Looking away from both, nothing is seen and the order stays.
    let away = camera(Vec3::new(-3.0, 2.0, 0.0), Vec3::new(-8.0, 2.0, 0.0), 0.3);
    assert_eq!(order(Some(away)), [0, 1, 0, 1]);
}

#[test]
fn a_texel_behind_a_wall_is_not_seen() {
    let scene = BakeScene {
        meshes: vec![floor(), wall()],
        ..Default::default()
    };
    let rays = rays::Rays::build(bake::triangles(&scene));
    let eye = Vec3::new(-3.0, 2.0, 0.0);
    let view = camera(eye, Vec3::new(2.0, 2.0, 0.0), 1.0);
    let front = Vec3::new(2.0, 2.0, 0.0);
    assert!(view.sees(&rays, front, Vec3::NEG_X));
    // The same spot facing away from the camera, and a point beyond the wall.
    assert!(!view.sees(&rays, front, Vec3::X));
    assert!(!view.sees(&rays, Vec3::new(3.0, 2.0, 0.0), Vec3::NEG_X));
    let blocker = quad(
        5,
        [
            Vec3::new(0.0, 0.0, -2.0),
            Vec3::new(0.0, 4.0, -2.0),
            Vec3::new(0.0, 4.0, 2.0),
            Vec3::new(0.0, 0.0, 2.0),
        ],
        Vec3::ONE,
    );
    let walled = BakeScene {
        meshes: vec![floor(), wall(), blocker],
        ..Default::default()
    };
    let rays = rays::Rays::build(bake::triangles(&walled));
    assert!(
        !view.sees(&rays, front, Vec3::NEG_X),
        "a wall stands between"
    );
}

#[test]
fn a_chart_with_no_texel_is_never_seen_and_the_bake_still_finishes() {
    // Lightmap UVs wholly outside [0, 1]: a lightmap with no texel (#1970's review).
    let mut outside = quad(
        3,
        [
            Vec3::new(-2.0, 3.0, -2.0),
            Vec3::new(-2.0, 3.0, 2.0),
            Vec3::new(0.0, 3.0, 2.0),
            Vec3::new(0.0, 3.0, -2.0),
        ],
        Vec3::ONE,
    );
    outside.lightmap_uvs.iter_mut().for_each(|uv| *uv += 5.0);
    let scene = BakeScene {
        meshes: vec![floor(), outside, wall()],
        ..Default::default()
    };
    let s = BakeSettings {
        samples: 2,
        samples_per_pass: 1,
        ..settings()
    };
    let (heads, points) = bake::receivers(&scene, &s);
    assert_eq!(heads.len(), 3, "the outside mesh still gets a lightmap");
    assert!(points[1].is_empty(), "with no texel");
    let view = camera(Vec3::new(-3.0, 2.0, 0.0), Vec3::new(2.0, 2.0, 0.0), 0.3);
    let progress = BakeProgress::default();
    let maps = progressive_on(&scene, &s, &progress, (Some(view), 2), &mut |_, _| {});
    let one_shot = bake::bake_on(&scene, &s, &BakeProgress::default(), 2);
    assert_eq!(maps.expect("finished"), one_shot.expect("finished"));
}
