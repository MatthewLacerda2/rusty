//! Montages (#1894): an interrupted one blends out from its weight and ends
//! once, a looping section holds until a jump, and a clip plays as a montage
//! with no graph.

use super::montage_tests::{count, rig, run, start, with_montages, UPPER};
use super::test_rig::*;
use super::*;
use crate::components::{ActiveMontage, MontageRefused, Motion, MONTAGE_BLEND_OUT, MONTAGE_END};

#[test]
fn an_interrupted_montage_blends_out_from_its_weight_and_ends_once() {
    let (m, mut anim) = (rig(), animator(Motion::Clip("Run"), None, 0.0));
    let g = with_montages(
        UPPER,
        r#"[{ "name": "Reload", "clip": "Reload", "slot": "UpperBody", "blend_in": 0.2 }]"#,
    );
    start(&mut anim, &g, &m, "Reload");
    run(&mut anim, &m, &g, 2);
    start(&mut anim, &g, &m, "Reload");
    let old = anim.montages.playing[0]
        .out
        .expect("the first one blends out");
    assert!(
        (old.from - 0.5).abs() < 1e-4,
        "from where it was, not from 1"
    );
    assert_eq!((old.duration, anim.montages.playing.len()), (0.2, 2));
    let mut events = run(&mut anim, &m, &g, 5);
    assert_eq!(
        anim.montages.playing.len(),
        1,
        "the interrupted one is gone"
    );
    assert_eq!(
        (
            count(&events, MONTAGE_BLEND_OUT),
            count(&events, MONTAGE_END)
        ),
        (1, 1)
    );
    assert!(anim.montages.stop(Some("UpperBody"), Some(0.0)));
    assert!(!anim.montages.stop(Some("UpperBody"), None), "nothing left");
    events.extend(run(&mut anim, &m, &g, 1));
    assert_eq!(
        (
            count(&events, MONTAGE_BLEND_OUT),
            count(&events, MONTAGE_END)
        ),
        (2, 2)
    );
    assert!(anim.montages.playing.is_empty());
}

#[test]
fn a_looping_section_repeats_until_a_jump_moves_on() {
    let (m, mut anim) = (rig(), animator(Motion::Clip("Run"), None, 0.0));
    let g = with_montages(
        "[]",
        r#"[{ "name": "Throw", "clip": "Reload", "blend_in": 0, "blend_out": 0,
        "sections": [{ "name": "Wind", "time": 0, "next": "Hold" },
                     { "name": "Hold", "time": 0.4, "next": "Hold" },
                     { "name": "Release", "time": 0.7 }] }]"#,
    );
    start(&mut anim, &g, &m, "Throw");
    let events = run(&mut anim, &m, &g, 100);
    let throw = &anim.montages.playing[0];
    assert_eq!(throw.section_name(), "Hold");
    assert!(
        (0.4..0.7).contains(&throw.time),
        "looping in Hold: {}",
        throw.time
    );
    assert!(events.is_empty(), "{events:?}");
    assert!(!anim.montages.jump("Hodl", None));
    assert!(anim.montages.jump("Release", None));
    assert_eq!(anim.montages.playing[0].time, 0.7);
    let events = run(&mut anim, &m, &g, 40);
    assert_eq!(events, vec![MONTAGE_BLEND_OUT, MONTAGE_END]);
}

#[test]
fn a_clip_plays_as_a_montage_in_the_default_slot_without_a_graph() {
    let m = rig();
    let flinch = ActiveMontage::resolve(None, &m.clips, "Reload", (None, None, 1.0)).unwrap();
    assert_eq!(
        (flinch.slot.name.as_str(), flinch.slot.mask.len()),
        ("DefaultSlot", 0)
    );
    assert_eq!(
        (flinch.montage.blend_in, flinch.montage.blend_out),
        (0.25, 0.25)
    );
    assert_eq!((flinch.length, flinch.section_name()), (2.0, "Default"));
    let unknown = ActiveMontage::resolve(None, &m.clips, "Nope", (None, None, 1.0));
    assert_eq!(unknown.unwrap_err(), MontageRefused::Unknown);
    let slot = ActiveMontage::resolve(None, &m.clips, "Reload", (Some("Upper"), None, 1.0));
    assert_eq!(slot.unwrap_err(), MontageRefused::NoSlot("Upper".into()));
    let mut anim = animator(Motion::Clip("Run"), None, 0.0);
    anim.montages.play(flinch);
    let g = AnimationGraph::default();
    let events = run(&mut anim, &m, &g, 60);
    assert_eq!(events, vec![MONTAGE_BLEND_OUT, MONTAGE_END]);
}

#[test]
fn a_save_or_play_stop_restore_leaves_every_slot_empty() {
    let m = rig();
    let mut anim = animator(Motion::Clip("Run"), None, 0.0);
    let flinch = ActiveMontage::resolve(None, &m.clips, "Reload", (None, None, 1.0)).unwrap();
    anim.montages.play(flinch);
    anim.montages.stop(None, None);
    let json = serde_json::to_string(&anim).unwrap();
    assert!(!json.contains("montage"), "{json}");
    let restored: AnimatorComponent = serde_json::from_str(&json).unwrap();
    assert!(
        restored.montages.is_empty(),
        "no montage and no pending event"
    );
}
