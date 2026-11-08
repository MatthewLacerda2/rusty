//! The lamp grid (#1883): a point is offered every lamp that can light it, and in
//! a big level only a few of the many.

use glam::Vec3;

use super::super::input::{BakeLight, LightShape};
use super::super::rng::Rng;
use super::grid::LightGrid;
use super::sources::lamp_weight;
use crate::scene::lighting::DiffuseModel;

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

#[test]
fn a_point_is_offered_every_lamp_that_reaches_it_and_few_others() {
    let lights = level();
    let grid = LightGrid::build(&lights);
    let mut rng = Rng::new(8, 0, 0);
    let mut offered = 0;
    for _ in 0..500 {
        let p = Vec3::new(
            rng.next_f32() * 110.0 - 5.0,
            rng.next_f32() * 6.0,
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
    assert!(
        offered / 500 < 30,
        "{} of 201 offered on average",
        offered / 500
    );
}

#[test]
fn far_off_nan_and_empty_points_get_only_the_unbounded_lamps() {
    let lights = level();
    let grid = LightGrid::build(&lights);
    for p in [Vec3::splat(1e4), Vec3::splat(-1e4), Vec3::NAN] {
        assert_eq!(grid.near(p).collect::<Vec<_>>(), vec![0], "{p}");
    }
    let nan = lamp(LightShape::Point {
        position: Vec3::NAN,
        range: 5.0,
    });
    let grid = LightGrid::build(&[nan]);
    assert_eq!(grid.near(Vec3::ZERO).collect::<Vec<_>>(), vec![0]);
    assert_eq!(LightGrid::build(&[]).near(Vec3::ZERO).count(), 0);
}
