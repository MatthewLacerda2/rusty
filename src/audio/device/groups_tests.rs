//! Mixer groups as kira tracks (#465): a voice heard through its group's volume,
//! mute, low-pass and reverb send.

use crate::audio::backend::{AudioBackend, PlayParams, VoiceId};
use crate::audio::device::backend::backend_tests::{flat, rig};
use crate::audio::mixer::{Filter, GroupId, GroupMix, GroupSettings};

const TONE: [f32; 4] = [0.5; 4];

/// The settled output level of a looping 0.5 voice in group `1` at `settings`.
fn group_level(settings: GroupSettings) -> f32 {
    let mut b = rig(&TONE);
    b.set_group(GroupId(1), &GroupMix::new(&settings, 1.0));
    let params = PlayParams {
        looping: true,
        group: GroupId(1),
        ..flat("clip")
    };
    assert!(b.play(VoiceId(1), &params));
    b.device().render(2048);
    b.device().render(1)[0].0
}

#[test]
fn a_voice_follows_its_group_volume_and_mute() {
    let half = GroupSettings {
        volume: 0.5,
        ..Default::default()
    };
    assert!((group_level(half) - 0.25).abs() < 1e-4);
    let muted = GroupSettings { mute: true, ..half };
    assert_eq!(group_level(muted), 0.0);
}

#[test]
fn a_low_pass_muffles_a_high_tone_and_an_open_one_does_not() {
    let nyquist: Vec<f32> = (0..64)
        .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
        .collect();
    let peak = |cutoff: f32| {
        let mut b = rig(&nyquist);
        let settings = GroupSettings {
            low_pass: Filter::new(cutoff, 0.0),
            ..Default::default()
        };
        b.set_group(GroupId(1), &GroupMix::new(&settings, 1.0));
        let params = PlayParams {
            looping: true,
            group: GroupId(1),
            ..flat("clip")
        };
        assert!(b.play(VoiceId(1), &params));
        b.device().render(4096);
        b.device()
            .render(64)
            .iter()
            .map(|f| f.0.abs())
            .fold(0.0, f32::max)
    };
    assert_eq!(
        peak(Filter::LOW_PASS_OPEN.cutoff),
        0.5,
        "open is transparent"
    );
    assert!(peak(500.0) < 0.05, "muffled");
}

#[test]
fn a_reverb_send_leaves_a_tail_after_the_voice_ends() {
    let tail = |send: f32| {
        let mut b = rig(&[0.5; 2048]);
        let settings = GroupSettings {
            reverb_send: send,
            ..Default::default()
        };
        b.set_group(GroupId(1), &GroupMix::new(&settings, 1.0));
        let params = PlayParams {
            group: GroupId(1),
            ..flat("clip")
        };
        assert!(b.play(VoiceId(1), &params));
        b.device().render(4096);
        let after = b.device().render(2048);
        after.iter().map(|f| f.0.abs()).fold(0.0, f32::max)
    };
    assert_eq!(tail(0.0), 0.0, "dry: silence after the clip");
    assert!(tail(1.0) > 0.001, "wet: the reverb rings on");
}
