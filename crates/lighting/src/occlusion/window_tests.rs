use glam::Vec3;

use crate::occlusion::bake_occlusion;
use crate::occlusion::test_level::block;
use rusty_scene::Scene;

const WINDOW: (Vec3, Vec3) = (Vec3::new(0.37, 1.13, -0.1), Vec3::new(1.37, 2.13, 0.1));
const CUBE: (Vec3, Vec3) = (Vec3::new(0.37, 1.13, -36.0), Vec3::new(1.37, 2.13, -35.0));

fn level() -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let (lo, hi) = WINDOW;
    let pieces = [
        (Vec3::new(-10.0, -6.0, lo.z), Vec3::new(lo.x, 9.0, hi.z)),
        (Vec3::new(hi.x, -6.0, lo.z), Vec3::new(10.0, 9.0, hi.z)),
        (Vec3::new(lo.x, -6.0, lo.z), Vec3::new(hi.x, lo.y, hi.z)),
        (Vec3::new(lo.x, hi.y, lo.z), Vec3::new(hi.x, 9.0, hi.z)),
    ];
    for (a, b) in pieces {
        block(&mut scene, "Wall", a, b);
    }
    for at in [Vec3::new(-80.0, -5.0, -80.0), Vec3::new(80.0, 5.0, 80.0)] {
        block(&mut scene, "Marker", at, at + Vec3::splat(0.2));
    }
    let cube = block(&mut scene, "Cube", CUBE.0, CUBE.1);
    (scene, cube)
}

fn sees_through_window(eye: Vec3) -> bool {
    let through = |p: Vec3| {
        [WINDOW.0.z, WINDOW.1.z].into_iter().all(|z| {
            let x = eye.lerp(p, (eye.z - z) / (eye.z - p.z));
            x.x > WINDOW.0.x && x.x < WINDOW.1.x && x.y > WINDOW.0.y && x.y < WINDOW.1.y
        })
    };
    let (lo, hi) = CUBE;
    let steps = 8;
    let at = |i: u32| i as f32 / steps as f32;
    (0..=steps).any(|i| {
        (0..=steps).any(|j| {
            let (u, v) = (at(i), at(j));
            let faces = [
                Vec3::new(lo.x + (hi.x - lo.x) * u, lo.y + (hi.y - lo.y) * v, hi.z),
                Vec3::new(lo.x, lo.y + (hi.y - lo.y) * u, lo.z + (hi.z - lo.z) * v),
                Vec3::new(lo.x + (hi.x - lo.x) * u, hi.y, lo.z + (hi.z - lo.z) * v),
                Vec3::new(lo.x + (hi.x - lo.x) * u, lo.y, lo.z + (hi.z - lo.z) * v),
            ];
            faces.into_iter().any(through)
        })
    })
}

#[test]
fn a_cube_through_a_window_is_kept_from_every_eye_at_a_dust2_cell() {
    let (scene, cube) = level();
    let data = bake_occlusion(&scene, 4).0.expect("the wall occludes");
    assert!(
        data.grid.cell > 3.8 && data.grid.cell < 4.0,
        "{:?}",
        data.grid
    );
    let object = data.object_of(cube).expect("baked");
    let (mut seen, mut missed) = (0, Vec::new());
    for (i, j, k) in (0..25).flat_map(|i| (0..9).flat_map(move |j| (0..25).map(move |k| (i, j, k))))
    {
        let eye = Vec3::new(
            -6.0 + 0.5 * i as f32,
            -0.9 + 0.6 * j as f32,
            0.3 + 0.48 * k as f32,
        );
        if !sees_through_window(eye) {
            continue;
        }
        seen += 1;
        if !data.sees(data.row_at(eye).expect("in the grid"), object) {
            missed.push(eye);
        }
    }
    assert!(
        seen > 100,
        "only {seen} eyes see the cube: the level is wrong"
    );
    assert!(
        missed.is_empty(),
        "{} of {seen} eyes see it culled: {:?}",
        missed.len(),
        &missed[..missed.len().min(5)]
    );
    assert!((0..data.row_count() as u32).any(|r| !data.sees(r, object)));
}
