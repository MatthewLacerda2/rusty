//! The lamp grid (#1883): a point is offered every lamp that can light it, and in
//! a big level only a few of the many.

use glam::Vec3;

use super::super::input::{BakeLight, LightShape};
use super::super::rng::Rng;
use super::grid::LightGrid;
use super::sources::lamp_weight;
use crate::lighting::DiffuseModel;

fn lamp(shape: LightShape) -> BakeLight {
    BakeLight {
        shape,
        radiance: Vec3::ONE,
        bakes_direct: true,
        mask_channel: None,
    }
}

/// 200 points and spots over a 100 m square, ranges 3–10 m, and a sun.
fn level() -> Vec<BakeLight> {
    let mut rng = Rng::new(7, 0, 0);
    let mut lights = vec![lamp(LightShape::Directional {
        direction: Vec3::NEG_Y,
    })];
    for k in 0..200 {
        let position = Vec3::new(rng.next_f32() * 100.0, 3.0, rng.next_f32() * 100.0);
        let range = 3.0 + rng.next_f32() * 7.0;
        lights.push(lamp(match k % 4 {
            0 => LightShape::Spot {
                position,
                direction: Vec3::NEG_Y,
                range,
                cos_inner: 0.9,
                cos_outer: 0.5,
            },
            _ => LightShape::Point { position, range },
        }));
    }
    lights
}

/// The level's floor plan, 100 m square and 6 m tall: what the bake clips to.
const FLOOR: (Vec3, Vec3) = (Vec3::ZERO, Vec3::new(100.0, 6.0, 100.0));

/// The average lamps `grid` offers a point of the level (and a little past it),
/// asserting every lamp of `lights` that lights a point is among them.
fn offered(lights: &[BakeLight], grid: &LightGrid) -> usize {
    let mut rng = Rng::new(8, 0, 0);
    let mut offered = 0;
    for _ in 0..500 {
        let p = Vec3::new(
            rng.next_f32() * 110.0 - 5.0,
            rng.next_f32() * 8.0 - 1.0,
            rng.next_f32() * 110.0 - 5.0,
        );
        let near: Vec<usize> = grid.near(p).collect();
        for (i, light) in lights.iter().enumerate() {
            for n in [Vec3::Y, Vec3::NEG_Y, Vec3::X] {
                let w = lamp_weight(light, p, n, DiffuseModel::HalfLambert);
                assert!(
                    w == 0.0 || near.contains(&i),
                    "lamp {i} lights {p} unoffered"
                );
            }
        }
        assert!(near.windows(2).all(|w| w[0] < w[1]), "in lamp order");
        offered += near.len();
    }
    offered / 500
}

#[test]
fn a_point_is_offered_every_lamp_that_reaches_it_and_few_others() {
    let lights = level();
    for clip in [None, Some(FLOOR)] {
        let n = offered(&lights, &LightGrid::build(&lights, clip));
        assert!(n < 30, "{n} of 201 offered on average, clip {clip:?}");
    }
}

#[test]
fn a_kilometre_lamp_does_not_stretch_the_cells_once_clipped() {
    let mut lights = level();
    let small = offered(&lights, &LightGrid::build(&lights, Some(FLOOR)));
    lights.push(lamp(LightShape::Point {
        position: Vec3::new(50.0, 500.0, 50.0),
        range: 1000.0,
    }));
    let n = offered(&lights, &LightGrid::build(&lights, Some(FLOOR)));
    assert!(
        n <= small + 2,
        "{n} offered beside the small lamps' {small}"
    );
    let unclipped = offered(&lights, &LightGrid::build(&lights, None));
    assert!(
        unclipped > 2 * n,
        "unclipped offers {unclipped}: the test bites"
    );
}

#[test]
fn far_off_nan_and_empty_points_get_only_the_unbounded_lamps() {
    let lights = level();
    let grid = LightGrid::build(&lights, None);
    for p in [Vec3::splat(1e4), Vec3::splat(-1e4), Vec3::NAN] {
        assert_eq!(grid.near(p).collect::<Vec<_>>(), vec![0], "{p}");
    }
    let nan = lamp(LightShape::Point {
        position: Vec3::NAN,
        range: 5.0,
    });
    let grid = LightGrid::build(&[nan], None);
    assert_eq!(grid.near(Vec3::ZERO).collect::<Vec<_>>(), vec![0]);
    assert_eq!(LightGrid::build(&[], None).near(Vec3::ZERO).count(), 0);
}
