//! The reverb bus (#469), heard through the capture backend: silent outside every
//! zone, a longer decay rings longer, and the pre-delay holds the tail back.

use super::feedback;
use crate::audio::backend::{AudioBackend, PlayParams, VoiceId};
use crate::audio::device::backend::backend_tests::{flat, rig};
use crate::audio::device::capture::RATE;
use crate::audio::mixer::{GroupId, GroupMix, GroupSettings};
use crate::components::{ReverbParams, ReverbPreset};

/// Long enough for the bus's retune tween and the pre-delay's glide to settle.
const SETTLE: usize = RATE as usize / 4;

/// The left channel of a 64-sample burst played through group `1` with the
/// reverb bus at `params` and the group sending `send` into it.
fn hear(params: ReverbParams, send: f32, frames: usize) -> Vec<f32> {
    let mut b = rig(&[0.5; 64]);
    let settings = GroupSettings {
        reverb_send: send,
        ..Default::default()
    };
    b.set_group(GroupId(1), &GroupMix::new(&settings, 1.0));
    b.set_reverb(&params);
    b.device().render(SETTLE);
    let burst = PlayParams {
        group: GroupId(1),
        ..flat("clip")
    };
    assert!(b.play(VoiceId(1), &burst));
    b.device().render(frames).iter().map(|f| f.0).collect()
}

/// The wet signal alone: the burst sent into the bus minus the burst kept dry.
fn wet(params: ReverbParams, frames: usize) -> Vec<f32> {
    let dry = hear(params, 0.0, frames);
    let sent = hear(params, 1.0, frames);
    sent.iter().zip(dry).map(|(s, d)| s - d).collect()
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s.abs()).fold(0.0, f32::max)
}

#[test]
fn feedback_grows_with_the_decay_and_never_rings_forever() {
    assert!(feedback(0.4) < feedback(2.8));
    assert!(feedback(2.8) < feedback(20.0));
    assert_eq!(feedback(1000.0), 0.98);
}

#[test]
fn outside_every_zone_the_bus_is_silent() {
    assert_eq!(peak(&wet(ReverbParams::DRY, RATE as usize / 2)), 0.0);
}

#[test]
fn a_longer_decay_rings_longer() {
    let late = |preset: ReverbPreset| {
        let params = ReverbParams {
            wet: 1.0,
            ..preset.params().unwrap()
        };
        let tail = wet(params, RATE as usize);
        peak(&tail[RATE as usize / 2..])
    };
    let (room, tunnel) = (late(ReverbPreset::Room), late(ReverbPreset::Tunnel));
    assert!(tunnel > 0.001, "a tunnel still rings after half a second");
    assert!(tunnel > 10.0 * room, "room {room} vs tunnel {tunnel}");
}

#[test]
fn the_pre_delay_holds_the_tail_back() {
    let onset = |pre_delay: f32| {
        let params = ReverbParams {
            pre_delay,
            wet: 1.0,
            ..ReverbPreset::Hall.params().unwrap()
        };
        let tail = wet(params, RATE as usize / 2);
        tail.iter()
            .position(|s| s.abs() > 1e-5)
            .expect("the tail sounds")
    };
    let shift = onset(0.1) as i64 - onset(0.0) as i64;
    let expected = (0.1 * RATE as f32) as i64;
    assert!((shift - expected).abs() <= 2, "shifted by {shift}");
}
