//! The eye side at a Dust2-sized cell (#1714): a 1 m cube seen through a 1 m
//! window is kept from every eye that sees it, though the grid's cells are 3.9 m.
//! A cell that large is sampled as 2 m cubes, so it sees what a 2 m cell does.

use glam::Vec3;

use super::bake_occlusion;
use super::test_level::block;
use rusty_scene::Scene;

/// The window in the wall at z = 0 (0.2 m thick), off any regular spacing.
const WINDOW: (Vec3, Vec3) = (Vec3::new(0.37, 1.13, -0.1), Vec3::new(1.37, 2.13, 0.1));
/// The cube 35 m behind it: from far out, the window shows it to a thin cone of
/// eyes, which one 3.9 m cell's six points miss (6 of 534 eyes, with the grid
/// sampled as one cube per cell).
const CUBE: (Vec3, Vec3) = (Vec3::new(0.37, 1.13, -36.0), Vec3::new(1.37, 2.13, -35.0));

/// The wall with its window, the cube, and two small markers 160 m apart that
/// make the static bounds Dust2-sized: the cube's id.
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

/// Whether some segment from `eye` to a point on the cube's surface passes the
/// window's front and back openings: a sampled, so one-way, test of sight.
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
    // Sampled, not kept everywhere: some cell behind the wall's far side hides it.
    assert!((0..data.row_count() as u32).any(|r| !data.sees(r, object)));
}
