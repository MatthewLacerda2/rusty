//! Emitters under MIS (#1883): a vertex's next-event sample and the bounce ray
//! after it each take a share of an emitter's light, and the shares sum to it.

use std::f32::consts::PI;

use glam::Vec3;

use super::super::bake::triangles;
use super::super::rays::Rays;
use super::super::rng::Rng;
use super::pick_tests::scene;
use super::sources::Picks;
use super::{cosine_sample, Tracer, T_MIN};

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
            .shadowed(point, arrived.map(|(t, d, l, _)| (t, d, l)))
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
