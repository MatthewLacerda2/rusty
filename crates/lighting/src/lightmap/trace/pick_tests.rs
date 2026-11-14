//! Next-event estimation (#1883): one importance-picked source per vertex, over
//! its pick probability, averages to the light of every source counted at once.

use glam::{Vec2, Vec3};

use super::super::bake::triangles;
use super::super::input::{BakeLight, BakeScene, LightShape};
use super::super::lights::area::{grid, AreaShape};
use super::super::rays::Rays;
use super::super::rng::Rng;
use super::super::tests::{floor, quad};
use super::sources::{Picks, Source};
use super::Tracer;

fn lamp(shape: LightShape, radiance: f32) -> BakeLight {
    BakeLight {
        shape,
        radiance: Vec3::splat(radiance),
        bakes_direct: false,
        mask_channel: None,
    }
}

/// Every kind of source near the origin, one point lamp behind a blocker.
pub(super) fn scene() -> BakeScene {
    let at = |x, y, z| Vec3::new(x, y, z);
    let point = |k: usize| LightShape::Point {
        position: at(k as f32 - 2.0, 2.0, 1.0),
        range: 6.0,
    };
    let mut lights: Vec<_> = (0..5).map(|k| lamp(point(k), 1.0 + k as f32)).collect();
    let spot = LightShape::Spot {
        position: at(0.0, 3.0, -1.0),
        direction: Vec3::NEG_Y,
        range: 8.0,
        cos_inner: 0.95,
        cos_outer: 0.8,
    };
    lights.push(lamp(spot, 6.0));
    let area = AreaShape {
        center: at(1.0, 2.5, 0.0),
        right: Vec3::X,
        up: Vec3::Z * 0.5,
        forward: Vec3::NEG_Y,
        range: 7.0,
    };
    lights.push(lamp(LightShape::Area(area), 3.0));
    lights.push(lamp(
        LightShape::Directional {
            direction: at(0.3, -1.0, 0.2).normalize(),
        },
        0.5,
    ));
    // A glowing tile over the origin and a slab shading the first lamp.
    let p = |x, z| Vec3::new(x, 1.5, z);
    let mut tile = quad(
        3,
        [p(-0.6, -0.4), p(-0.2, -0.4), p(-0.2, 0.0), p(-0.6, 0.0)],
        Vec3::ONE,
    );
    tile.runs[0].emissive = Vec3::splat(4.0);
    let s = |x, z| Vec3::new(x, 1.0, z);
    let slab = quad(
        4,
        [s(-2.5, 0.0), s(-1.5, 0.0), s(-1.5, 1.0), s(-2.5, 1.0)],
        Vec3::ONE,
    );
    BakeScene {
        meshes: vec![floor(), tile, slab],
        lights,
        sky: Vec3::ZERO.into(),
        ..Default::default()
    }
}

#[test]
fn one_picked_lamp_averages_to_every_lamp_counted() {
    let scene = scene();
    let rays = Rays::build(triangles(&scene));
    let tracer = Tracer::new(&scene, &rays);
    let (point, normal) = (Vec3::new(0.1, 1e-3, 0.2), Vec3::Y);
    // Every lamp counted once, an Area panel over a fine grid of points.
    let exact: Vec3 = (scene.lights.iter())
        .map(|light| match light.shape {
            LightShape::Area(_) => {
                (grid().map(|u| tracer.lamp(light, point, normal, u, 1.0 / 64.0).0)).sum()
            }
            _ => tracer.lamp(light, point, normal, Vec2::ZERO, 1.0).0,
        })
        .sum();
    let (mut rng, mut picks) = (Rng::new(5, 0, 0), Picks::default());
    let draws = 200_000;
    let mut once = || {
        tracer
            .next_event(point, normal, (true, false), &mut rng, &mut picks)
            .0
    };
    let sampled = (0..draws).map(|_| once()).sum::<Vec3>() / draws as f32;
    let err = (sampled - exact).abs().max_element();
    assert!(err < exact.x * 0.02, "{sampled} vs {exact}");
}

#[test]
fn a_vertex_no_source_reaches_draws_nothing_and_lamps_can_be_left_out() {
    let scene = scene();
    let rays = Rays::build(triangles(&scene));
    let tracer = Tracer::new(&scene, &rays);
    let (mut rng, mut picks) = (Rng::new(5, 0, 0), Picks::default());
    // Facing straight down, under everything: nothing lies in front of it.
    let below = Vec3::new(0.0, -1.0, 0.0);
    let lit = tracer.next_event(below, Vec3::NEG_Y, (true, true), &mut rng, &mut picks);
    assert_eq!(lit.0, Vec3::ZERO);
    let up = Vec3::new(0.0, 0.5, 0.0);
    tracer.next_event(up, Vec3::Y, (false, true), &mut rng, &mut picks);
    assert_eq!(
        picks.list,
        vec![(Source::Emitter(0), picks.total)],
        "lamps left out"
    );
    tracer.next_event(up, Vec3::Y, (true, false), &mut rng, &mut picks);
    assert!(!picks
        .list
        .iter()
        .any(|(s, _)| matches!(s, Source::Emitter(_))));
}
