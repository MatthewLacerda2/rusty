use std::time::Duration;

use super::format::eta;
use super::rate::{Window, WINDOW};
use super::tests::{lightmaps, logged};
use super::{Progress, Step};

const TOTAL: u64 = 12_000;

fn done_at(t: u64) -> u64 {
    match t {
        0..=60 => 100 * t,
        _ => 6_000 + 10 * (t - 60),
    }
}

fn etas() -> Vec<(u64, u64)> {
    let mut window = Window::default();
    let mut out = Vec::new();
    for t in (0..=600).step_by(5) {
        let (now, done) = (Duration::from_secs(t), done_at(t));
        window.note(now, done);
        let p = Progress {
            step: Step::Lightmaps,
            done,
            total: TOTAL,
            meshes: None,
        };
        if let Some(left) = window.from().and_then(|from| eta(from, now, &p)) {
            out.push((t, left.as_secs_f64().round() as u64));
        }
    }
    out
}

#[test]
fn a_cheap_then_expensive_step_settles_within_one_window() {
    let settled = 60 + WINDOW.as_secs();
    let truth = |t| (TOTAL - done_at(t)) / 10;
    let etas = etas();
    // Before the window fills, the rate is the step's since its start.
    assert!(etas.contains(&(30, 90)), "{etas:?}");
    for &(t, left) in etas.iter().filter(|(t, _)| *t >= settled) {
        assert_eq!(left, truth(t), "at {t} s");
    }
    for &(t, left) in etas.iter().filter(|(t, _)| *t > 60) {
        assert!(left <= truth(t), "at {t} s: {left} s over {}", truth(t));
    }
    let after: Vec<u64> = (etas.iter().filter(|(t, _)| *t >= settled))
        .map(|&(_, l)| l)
        .collect();
    assert!(after.windows(2).all(|w| w[1] <= w[0]), "{after:?}");
}

#[test]
fn a_new_step_measures_its_own_rate_from_scratch() {
    let lines = logged(|log, at| {
        at(0);
        log.observe(lightmaps(0, 1000, (0, 2)));
        at(300);
        log.observe(lightmaps(1000, 1000, (2, 2)));
        log.observe(Progress {
            step: Step::Occlusion,
            done: 0,
            total: 100,
            meshes: None,
        });
        at(310);
        log.observe(Progress {
            step: Step::Occlusion,
            done: 50,
            total: 100,
            meshes: None,
        });
    });
    let last = &lines.last().expect("lines").text;
    assert_eq!(
        last,
        "[Lighting] 5:10 occlusion 50% (50/100 cells), ~10 s left"
    );
}
