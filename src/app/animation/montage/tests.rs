//! Montages (#1894): a masked slot leaves the rest of the body to the layers,
//! and weights ramp over the blends while markers and the two engine events
//! fire once each. The helpers here serve every montage test.

use super::super::test_rig::*;
use super::super::*;
use crate::asset::animation_graph::{GraphSlot, Montage};
use crate::components::{ActiveMontage, Motion, MONTAGE_BLEND_OUT, MONTAGE_END};

pub(super) const DT: f32 = 0.05;

/// The rig's graph with `slots` and `montages` declared as JSON.
pub(super) fn with_montages(slots: &str, montages: &str) -> AnimationGraph {
    let mut g = graph(vec![node("Run", "Run")], Vec::new());
    g.slots = serde_json::from_str::<Vec<GraphSlot>>(slots).unwrap();
    g.montages = serde_json::from_str::<Vec<Montage>>(montages).unwrap();
    g.validate().unwrap();
    g
}

pub(super) fn start(
    anim: &mut AnimatorComponent,
    g: &AnimationGraph,
    m: &MeshComponent,
    name: &str,
) {
    let montage = ActiveMontage::resolve(Some(g), &m.clips, name, (None, None, 1.0)).unwrap();
    anim.montages.play(montage);
}

/// Step `n` times; the montage engine events and markers fired, in order.
pub(super) fn run(
    anim: &mut AnimatorComponent,
    m: &MeshComponent,
    g: &AnimationGraph,
    n: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    for _ in 0..n {
        let fired = step(anim, Some(m), Some(g), DT).events;
        out.extend(fired.into_iter().map(|e| e.name).filter(|n| n != END_EVENT));
    }
    out
}

pub(super) fn count(events: &[String], name: &str) -> usize {
    events.iter().filter(|e| *e == name).count()
}

pub(super) fn rig() -> MeshComponent {
    let all = [HIPS, SPINE, LEG, HEAD];
    mesh(vec![
        slide("Run", &all, 0.0, 2.0, 1.0),
        slide("Reload", &all, 0.0, -4.0, 2.0),
    ])
}

pub(super) const UPPER: &str = r#"[{ "name": "UpperBody", "mask": ["spine"] }]"#;

#[test]
fn an_upper_body_montage_leaves_the_legs_to_the_base_layer() {
    let (m, mut anim) = (rig(), animator(Motion::Clip("Run"), None, 0.5));
    let g = with_montages(
        UPPER,
        r#"[{ "name": "Reload", "clip": "Reload", "slot": "UpperBody", "blend_in": 0 }]"#,
    );
    start(&mut anim, &g, &m, "Reload");
    anim.montages.playing[0].time = 1.0;
    let pose = sample_pose(&anim, &m, Some(&g)).unwrap().1;
    let xs: Vec<f32> = pose.iter().map(|j| j.unwrap().translation.x).collect();
    // Run is at +1 everywhere, Reload at -2: the spine and head take Reload.
    assert_eq!(xs, vec![1.0, -2.0, 1.0, -2.0]);
}

#[test]
fn the_weight_ramps_in_then_out_and_both_events_fire_once() {
    let (m, mut anim) = (rig(), animator(Motion::Clip("Run"), None, 0.0));
    let g = with_montages(
        UPPER,
        r#"[{ "name": "Reload", "clip": "Reload", "slot": "UpperBody",
        "blend_in": 0.2, "blend_out": 0.4,
        "events": [{ "time": 0.9, "name": "MagOut" }, { "time": 1.6, "name": "MagIn" }] }]"#,
    );
    start(&mut anim, &g, &m, "Reload");
    run(&mut anim, &m, &g, 2);
    assert!(
        (anim.montages.playing[0].weight() - 0.5).abs() < 1e-4,
        "half way in"
    );
    let mut events = run(&mut anim, &m, &g, 2);
    assert!((anim.montages.playing[0].weight() - 1.0).abs() < 1e-4);
    // It starts blending out 0.4 s before the clip's 2 s end.
    while !events.iter().any(|e| e == MONTAGE_BLEND_OUT) {
        events.extend(run(&mut anim, &m, &g, 1));
    }
    let blending = &anim.montages.playing[0];
    assert!((blending.time - 1.6).abs() < 0.06, "at {}", blending.time);
    assert_eq!(blending.weight(), 1.0);
    events.extend(run(&mut anim, &m, &g, 4));
    assert!(
        (anim.montages.playing[0].weight() - 0.5).abs() < 1e-3,
        "half way out"
    );
    events.extend(run(&mut anim, &m, &g, 20));
    assert!(anim.montages.playing.is_empty(), "gone once blended out");
    for name in ["MagOut", "MagIn", MONTAGE_BLEND_OUT, MONTAGE_END] {
        assert_eq!(count(&events, name), 1, "{name} in {events:?}");
    }
    let at = |n: &str| events.iter().position(|e| e == n).unwrap();
    assert!(at("MagOut") < at("MagIn") && at(MONTAGE_BLEND_OUT) < at(MONTAGE_END));
}
