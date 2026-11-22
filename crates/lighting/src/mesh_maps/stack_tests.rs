use glam::{Vec2, Vec3};

use super::fixtures::quad;
use super::overlap_tests::{bake, shared, square, UV};
use super::{MeshMap, MeshSource, Settings};

fn mirrored_floor() -> MeshSource {
    let mut m = MeshSource::default();
    let v = Vec3::new;
    let right = [v(0., 0., 0.), v(0., 0., 1.), v(1., 0., 1.), v(1., 0., 0.)];
    quad(&mut m, right, UV, 0);
    let left = [v(0., 0., 1.), v(0., 0., 0.), v(-1., 0., 0.), v(-1., 0., 1.)];
    quad(
        &mut m,
        left,
        (Vec2::new(0.75, 0.25), Vec2::new(0.25, 0.75)),
        0,
    );
    let near = [
        v(-1., 0., 0.),
        v(1., 0., 0.),
        v(1., 0.5, 0.),
        v(-1., 0.5, 0.),
    ];
    let far = [
        v(1., 0., 1.),
        v(-1., 0., 1.),
        v(-1., 0.5, 1.),
        v(1., 0.5, 1.),
    ];
    quad(&mut m, near, (Vec2::ZERO, Vec2::new(0.2, 0.1)), 0);
    quad(&mut m, far, (Vec2::new(0.0, 0.85), Vec2::new(0.2, 0.95)), 0);
    m
}

#[test]
fn mirrored_halves_open_alike_bake_one_half_on_every_shared_texel() {
    let mesh = mirrored_floor();
    let maps = bake(&mesh, &[MeshMap::Ao, MeshMap::Position]);
    let ao: Vec<f32> = shared(&maps[0].1).map(|t| t[0]).collect();
    assert!(
        ao.iter().any(|&a| a < 0.9),
        "the walls shade the floor: {ao:?}"
    );
    let sides: Vec<bool> = shared(&maps[1].1).map(|p| p[0] > 0.5).collect();
    assert!(sides.iter().all(|&s| s == sides[0]), "speckled: {sides:?}");
}

#[test]
fn a_deep_stack_of_faces_on_one_square_is_told_and_a_shallow_one_is_not() {
    let stack = |n: usize| {
        let mut m = MeshSource::default();
        (0..n).for_each(|k| square(&mut m, k as f32 * 0.01));
        super::Baked::new(&m, &Settings::new(16)).unwrap().2
    };
    assert_eq!(stack(2), None);
    let deep = stack(10).expect("ten faces on one square are told");
    assert_eq!(deep.faces, 10 * deep.texels, "{deep:?}");
}
