//! Emitters under MIS (#1883): a vertex's next-event sample and the bounce ray
//! after it each take a share of an emitter's light, and the shares sum to it,
//! past a mesh only one of a shadow ray or a bounce ray would otherwise meet.

use std::f32::consts::PI;

use glam::Vec3;

use super::super::bake::triangles;
use super::super::input::{BakeMesh, BakeScene};
use super::super::rays::Rays;
use super::super::rng::Rng;
use super::super::tests::{floor, quad};
use super::pick_tests::scene;
use super::sources::Picks;
use super::{cosine_sample, Tracer, T_MIN};
use rusty_components::ShadowCasting;

#[test]
fn an_emitters_two_mis_shares_sum_to_its_light() {
    let scene = scene();
    let rays = Rays::build(triangles(&scene));
    let tracer = Tracer::new(&scene, &rays);
    let (point, normal) = (Vec3::new(0.1, 1e-3, 0.2), Vec3::Y);
    let (mut rng, mut picks) = (Rng::new(6, 0, 0), Picks::default());
    let draws = 200_000;
    let emitter = &tracer.sources.emitters[0];
    let mut exact = Vec3::ZERO;
    for _ in 0..draws {
        let u = [rng.next_f32(), rng.next_f32(), rng.next_f32()];
        let arrived = emitter.arrival(point, normal, u);
        exact += tracer
            .unhidden(point, arrived.map(|(t, d, l, _)| (t, d, l)))
            .0;
    }
    // The next-event share, then the bounce rays' share against the same picks.
    let mut nee = Vec3::ZERO;
    for _ in 0..draws {
        nee += tracer
            .next_event(point, normal, (false, true), &mut rng, &mut picks)
            .0;
    }
    let mut bounce = Vec3::ZERO;
    for _ in 0..draws {
        let dir = cosine_sample(normal, &mut rng);
        if let Some(hit) = rays.closest(point, dir, T_MIN, f32::MAX) {
            if tracer.surface(&hit, dir).is_some() {
                bounce += tracer.glow(&hit, point, normal, dir, &picks);
            }
        }
    }
    let (exact, sum) = (exact / PI, (nee / PI + bounce) / draws as f32);
    let exact = exact / draws as f32;
    assert!(
        nee.x > 0.0 && bounce.x > 0.0,
        "both strategies take a share"
    );
    assert!(
        (sum - exact).abs().max_element() < exact.x * 0.03,
        "{sum} vs {exact}"
    );
}

/// A dark room: the floor, a bright tile 1.5 m over it, and `between` 0.8 m up,
/// under the tile, with its *Cast Shadows* at `mode`.
fn veiled(between: Option<ShadowCasting>) -> BakeScene {
    let p = |y, x, z| Vec3::new(x, y, z);
    let pane = |y: f32| {
        [
            p(y, -1.0, -1.0),
            p(y, 0.5, -1.0),
            p(y, 0.5, 0.5),
            p(y, -1.0, 0.5),
        ]
    };
    let mut tile = quad(3, pane(1.5), Vec3::ONE);
    tile.runs[0].emissive = Vec3::splat(4.0);
    let mut meshes: Vec<BakeMesh> = vec![floor(), tile];
    if let Some(mode) = between {
        let mut veil = quad(4, pane(0.8), Vec3::splat(0.5));
        veil.cast_shadows = mode;
        meshes.push(veil);
    }
    BakeScene {
        meshes,
        sky: Vec3::ZERO.into(),
        ..Default::default()
    }
}

/// The bounce part a floor texel under the tile bakes in `scene`.
fn under(scene: &BakeScene) -> f32 {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    let mut rng = Rng::new(9, 0, 0);
    let at = Vec3::new(-0.3, 0.0, -0.2);
    tracer.texel(at, Vec3::Y, 4000, 2, &mut rng).indirect.x
}

#[test]
fn an_emitter_is_hidden_by_what_a_bounce_ray_sees_not_by_what_casts() {
    // A Shadows-`Off` pane is seen by bounce rays, so it must hide the tile from
    // the next-event sample too: the floor bakes as under a casting pane. A
    // `ShadowsOnly` proxy is seen by neither: the floor bakes as under nothing.
    let cases = [
        (ShadowCasting::Off, Some(ShadowCasting::On)),
        (ShadowCasting::ShadowsOnly, None),
    ];
    for (mode, reference) in cases {
        let (got, want) = (under(&veiled(Some(mode))), under(&veiled(reference)));
        assert!(
            (got - want).abs() <= want * 0.01,
            "{mode:?}: {got} against {want}"
        );
    }
}
