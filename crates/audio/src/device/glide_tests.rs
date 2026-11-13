//! A voice's parameters glide per sample (#1693): a mix the game retunes once a
//! frame never jumps sample to sample, through the whole kira graph, and a
//! one-shot still starts on its targets with no fade-in.

use super::*;
use crate::backend::{AudioBackend, PlayParams, VoiceId, VoiceMix};
use crate::device::backend::backend_tests::{flat, rig};
use crate::device::capture::RATE;
use crate::device::decode::CachedClip;

/// One game frame at 25 FPS, in device frames: how long a retune holds.
const FRAME: usize = 1_920;

/// Play the mono `clip` looping at mix `a`, retune it to `b` and back every
/// [`FRAME`] for ten frames, and return everything the device rendered.
fn toggled(clip: &[f32], a: VoiceMix, b: VoiceMix) -> Vec<(f32, f32)> {
    toggled_clip(CachedClip::new(1, RATE, clip.to_vec()), a, b)
}

/// [`toggled`], for any clip.
fn toggled_clip(clip: CachedClip, a: VoiceMix, b: VoiceMix) -> Vec<(f32, f32)> {
    let mut backend = rig(&[]);
    backend.cache().insert("clip", clip);
    let params = PlayParams {
        mix: a,
        looping: true,
        ..flat("clip")
    };
    assert!(backend.play(VoiceId(1), &params));
    let mut out = Vec::new();
    for i in 0..10 {
        backend.set_mix(VoiceId(1), if i % 2 == 0 { &a } else { &b });
        out.extend(backend.device().render(FRAME));
    }
    out
}

/// The largest change of `f` between samples `gap` apart.
fn largest_jump(out: &[(f32, f32)], gap: usize, f: impl Fn(&(f32, f32)) -> f32) -> f32 {
    out.windows(gap + 1)
        .map(|w| (f(&w[gap]) - f(&w[0])).abs())
        .fold(0.0, f32::max)
}

#[test]
fn a_gain_toggled_every_frame_never_jumps() {
    let loud = VoiceMix::FLAT;
    let quiet = VoiceMix { gain: 0.1, ..loud };
    let out = toggled(&[0.5], loud, quiet);
    let jump = largest_jump(&out, 1, |f| f.0);
    assert!(jump < 0.005, "a 0.45 step glides: largest jump {jump}");
    assert!(out.iter().any(|f| f.0 < 0.06), "the gain still reaches 0.1");
}

#[test]
fn a_pan_toggled_every_frame_never_jumps() {
    let left = VoiceMix {
        pan: -1.0,
        ..VoiceMix::FLAT
    };
    let right = VoiceMix { pan: 1.0, ..left };
    let out = toggled(&[0.5], left, right);
    for side in [|f: &(f32, f32)| f.0, |f: &(f32, f32)| f.1] {
        let jump = largest_jump(&out, 1, side);
        assert!(jump < 0.005, "a hard-left to hard-right pan glides: {jump}");
    }
}

#[test]
fn a_spatial_blend_toggled_every_frame_never_jumps() {
    // A stereo clip, all left: blend 0 keeps (1, 0), blend 1 downmixes to (½, ½).
    let flat = VoiceMix::FLAT;
    let spatial = VoiceMix {
        spatial_blend: 1.0,
        ..flat
    };
    let out = toggled_clip(CachedClip::new(2, RATE, vec![1.0, 0.0]), flat, spatial);
    for side in [|f: &(f32, f32)| f.0, |f: &(f32, f32)| f.1] {
        let jump = largest_jump(&out, 1, side);
        assert!(jump < 0.005, "a 2D to 3D blend glides: {jump}");
    }
    assert!(out.iter().any(|f| f.1 > 0.49), "and still downmixes");
}

#[test]
fn a_low_pass_toggled_every_frame_never_jumps() {
    // A tone at Nyquist: the open filter passes all of it, the closed one almost
    // none, so its level is the filter's state. Compare samples of one polarity.
    let open = VoiceMix::FLAT;
    let closed = VoiceMix {
        low_pass: 800.0,
        ..open
    };
    let out = toggled(&[0.5, -0.5], open, closed);
    let jump = largest_jump(&out, 2, |f| f.0.abs());
    assert!(jump < 0.02, "the muffle glides in and out: {jump}");
    assert!(out.iter().any(|f| f.0.abs() < 0.05), "and still closes");
}

#[test]
fn a_one_shot_starts_on_its_targets() {
    let clip: Vec<f32> = (0..256).map(|i| (i as f32 * 0.37).sin()).collect();
    let mix = VoiceMix {
        gain: 0.5,
        pan: 0.3,
        spatial_blend: 1.0,
        low_pass: 800.0,
        ..VoiceMix::FLAT
    };
    let mut b = rig(&clip);
    let params = PlayParams {
        mix,
        ..flat("clip")
    };
    assert!(b.play(VoiceId(1), &params));
    let out = b.device().render(256);
    let a = LowPass::coefficient(mix.low_pass, 1.0 / f64::from(RATE));
    let mut filter = LowPass::default();
    for (s, got) in clip.iter().zip(&out) {
        let (l, r) = filter.process(a, mix_frame(&[*s], mix.pan, mix.spatial_blend));
        assert_eq!(*got, (l * mix.gain, r * mix.gain), "no fade-in");
    }
}

#[test]
fn a_glide_lands_on_its_target_and_recovers_from_a_nan() {
    let at = |v: f32| Glide {
        gain: v,
        pan: v,
        blend: v,
        low_pass: v,
        doppler: v,
    };
    let mut g = at(0.0);
    for _ in 0..100_000 {
        g.toward(&at(1.0), 0.002);
    }
    assert_eq!(g, at(1.0), "lands exactly, never stalls short");
    assert_eq!(g.filter(), None, "a landed open filter is bypassed");
    g.toward(&at(f32::NAN), 0.002);
    g.toward(&at(0.5), 0.002);
    assert_eq!(g, at(0.5), "the glide recovers the sample its target does");
}
