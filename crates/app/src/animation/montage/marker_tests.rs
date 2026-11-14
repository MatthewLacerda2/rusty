//! Montages (#1894): a stopped or interrupted montage's markers fall silent
//! as it fades, one ending on its own keeps firing them, and each engine
//! event says whose montage it is and whether it was interrupted.

use super::super::test_rig::*;
use super::super::*;
use super::tests::{rig, start, with_montages, DT, UPPER};
use rusty_components::{MontageEvent, Motion, MONTAGE_BLEND_OUT, MONTAGE_END};

/// Reload's 2 s clip: `MagIn` at 1.6 s, `Late` inside its 0.2 s blend out,
/// `Start` on its first frame.
const RELOAD: &str = r#"[{ "name": "Reload", "clip": "Reload", "slot": "UpperBody",
    "blend_in": 0.2, "blend_out": 0.2,
    "events": [{ "time": 0, "name": "Start" }, { "time": 1.6, "name": "MagIn" },
               { "time": 1.95, "name": "Late" }] }]"#;

/// Step `n` times; every event fired but the base layer's `"End"`.
fn fire(
    anim: &mut AnimatorComponent,
    m: &MeshComponent,
    g: &AnimationGraph,
    n: usize,
) -> Vec<Crossed> {
    let mut out = Vec::new();
    for _ in 0..n {
        let fired = step(anim, Some(m), Some(g), DT).events;
        out.extend(fired.into_iter().filter(|e| e.name != END_EVENT));
    }
    out
}

fn names(events: &[Crossed]) -> Vec<&str> {
    events.iter().map(|e| e.name.as_str()).collect()
}

fn reload(interrupted: bool) -> Option<MontageEvent> {
    Some(MontageEvent {
        montage: "Reload".into(),
        slot: "UpperBody".into(),
        interrupted,
    })
}

#[test]
fn a_montage_stopped_before_a_marker_never_fires_it() {
    let (m, g) = (rig(), with_montages(UPPER, RELOAD));
    let mut anim = animator(Motion::Clip("Run"), None, 0.0);
    start(&mut anim, &g, &m, "Reload");
    let before = fire(&mut anim, &m, &g, 31); // 1.55 s
    assert_eq!(names(&before), vec!["Start"]);
    assert!(anim.montages.stop(Some("UpperBody"), None));
    let after = fire(&mut anim, &m, &g, 20);
    assert_eq!(names(&after), vec![MONTAGE_BLEND_OUT, MONTAGE_END]);
    assert!(after.iter().all(|e| e.montage == reload(true)), "{after:?}");
}

#[test]
fn a_montage_ending_on_its_own_fires_markers_in_its_blend_out() {
    let (m, g) = (rig(), with_montages(UPPER, RELOAD));
    let mut anim = animator(Motion::Clip("Run"), None, 0.0);
    start(&mut anim, &g, &m, "Reload");
    let events = fire(&mut anim, &m, &g, 60);
    let want = vec!["Start", "MagIn", MONTAGE_BLEND_OUT, "Late", MONTAGE_END];
    assert_eq!(names(&events), want);
    let info: Vec<_> = events.iter().map(|e| e.montage.clone()).collect();
    assert_eq!(info, vec![None, None, reload(false), None, reload(false)]);
}

#[test]
fn two_plays_in_one_step_leave_only_the_second_firing_markers() {
    let (m, g) = (rig(), with_montages(UPPER, RELOAD));
    let mut anim = animator(Motion::Clip("Run"), None, 0.0);
    start(&mut anim, &g, &m, "Reload");
    start(&mut anim, &g, &m, "Reload");
    let events = fire(&mut anim, &m, &g, 10);
    let interrupted = |name: &str| {
        let of = |e: &&Crossed| e.name == name;
        events
            .iter()
            .filter(of)
            .map(|e| e.montage.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(interrupted("Start"), vec![None], "{events:?}");
    assert_eq!(interrupted(MONTAGE_BLEND_OUT), vec![reload(true)]);
    assert_eq!(interrupted(MONTAGE_END), vec![reload(true)]);
    assert_eq!(anim.montages.playing.len(), 1);
}
