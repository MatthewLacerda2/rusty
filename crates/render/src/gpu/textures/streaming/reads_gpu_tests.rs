use super::gpu_tests::{boxes, draw, dropped, gradient, settle, RES};
use super::reads::IN_FLIGHT;

#[test]
fn gpu_walking_closer_never_has_more_reads_in_flight_than_the_cap() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    r.stream_textures(true);
    let names = ["walk_a", "walk_b", "walk_c", "walk_d", "walk_e", "walk_f"];
    let maps: Vec<_> = names.iter().map(|n| gradient(n, 256)).collect();
    let all: Vec<_> = maps.iter().collect();
    let at = |z: f32| boxes(&maps.iter().map(|m| (m, z)).collect::<Vec<_>>());
    settle(&mut r, &at(200.0), RES, &all);
    for step in 0..60 {
        draw(&mut r, &at(200.0 - step as f32 * 3.3), RES);
        let flying = r.streaming.pending();
        assert!(flying <= IN_FLIGHT, "step {step}: {flying} reads in flight");
    }
    settle(&mut r, &at(2.0), RES, &all);
    for map in &maps {
        assert_eq!(dropped(&r, map), 0, "{map:?}: every level back");
    }
}
