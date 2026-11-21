//! The room, by the sample: its two sides, its tail, and what it does at
//! each extreme of its knobs.

use super::*;

mod predelay;

/// A full-scale impulse then silence, dead centre — the classic reverb
/// probe.
fn impulse(n: usize) -> Stereo {
    let mut mono = vec![0.0; n];
    mono[0] = 1.0;
    Stereo::centred(mono)
}

/// Total energy in a window, the measure a tail is judged by.
fn energy(buf: &[f32]) -> f32 {
    buf.iter().map(|s| s.abs()).sum()
}

/// The reverb's answer to a centred impulse.
fn rung(n: usize, size: f32, damp: f32, mix: f32) -> Stereo {
    let mut buf = impulse(n);
    apply(&mut buf, size, damp, 0.0, mix, 44_100.0);
    buf
}

/// The first reflection, by the number: the send itself, arriving at the
/// shortest comb's delay, on both sides, **the same way up it went in**.
///
/// This is the one assertion here that nothing relative can make. The
/// send being the *average* of the channels and not their sum is a claim
/// about an absolute level — doubling it doubles every wet sample, and
/// every comparison between two renders agrees with itself either way.
/// The **sign** is worse: a blend that subtracted instead of adding
/// negates the fully-wet render and the half-wet one alike, so even
/// "half of the way across" above cannot see it. That is the reverb's
/// version of #60.
///
/// So it is checked against numbers that are exact rather than measured.
/// A centred impulse of `1.0` folds down to `1.0` and is scaled by
/// [`FIXED_GAIN`]; the combs' lines are still empty, so the shortest one
/// hands that straight back at [`COMB_LENGTHS`]`[0]`, and the four
/// allpasses each negate what they are given — four negations being none.
/// So exactly `+FIXED_GAIN` comes out, and on the right it comes out
/// [`STEREO_SPREAD`] samples later, which pins the offset too.
#[test]
fn the_first_reflection_is_the_send_itself_the_same_way_up() {
    let buf = rung(44_100, 0.8, 0.5, 1.0);
    let first = |channel: &[f32]| {
        channel
            .iter()
            .enumerate()
            .find(|(_, sample)| sample.abs() > 1e-9)
            .map(|(at, sample)| (at, *sample))
            .expect("the room answered")
    };
    assert_eq!(
        first(&buf.l),
        (COMB_LENGTHS[0], FIXED_GAIN),
        "the left tail is the send, not twice it and not its negative"
    );
    assert_eq!(
        first(&buf.r),
        (COMB_LENGTHS[0] + STEREO_SPREAD, FIXED_GAIN),
        "and the right is the same, a stereo spread later"
    );
}

#[test]
fn an_impulse_becomes_a_decaying_tail() {
    let buf = rung(88_200, 0.8, 0.5, 1.0);
    for channel in [&buf.l, &buf.r] {
        let early = energy(&channel[..22_050]);
        let late = energy(&channel[66_150..]);
        assert!(early > 0.5, "the reverb produced almost nothing ({early})");
        assert!(late < early, "the tail must decay: {early} → {late}");
        assert!(channel.iter().all(|s| s.abs() <= 1.0), "and never clip");
    }
}

/// What the stereo spread is for: one room, but the two sides of it are
/// different signals rather than one signal twice. Both carry a tail — a
/// bank that had silently failed to build would read as "different" too.
#[test]
fn the_two_sides_of_the_room_are_not_the_same_tail() {
    let buf = rung(44_100, 0.8, 0.5, 1.0);
    assert_ne!(buf.l, buf.r);
    let (left, right) = (energy(&buf.l[4410..]), energy(&buf.r[4410..]));
    assert!(left > 0.5 && right > 0.5, "{left} / {right}");
    // Neither side is favoured: the same twelve filters, offset, so the
    // tails carry comparable energy.
    assert!(
        (left - right).abs() < left.max(right) * 0.5,
        "one side is far louder: {left} / {right}"
    );
}

/// A hard-panned source still reaches both sides of the room, because the
/// send is the mix rather than the channel. A reverb that only wet the
/// channel it was given would put a panned instrument's tail in the same
/// place as the instrument, which is not what a room does.
#[test]
fn a_one_sided_source_still_rings_on_both_sides() {
    let mut buf = Stereo::silence(44_100);
    buf.l[0] = 1.0;
    apply(&mut buf, 0.8, 0.5, 0.0, 1.0, 44_100.0);
    assert!(energy(&buf.r[4410..]) > 0.2, "the right side stayed dry");
}

#[test]
fn a_bigger_room_rings_longer() {
    let tail = |size: f32| energy(&rung(88_200, size, 0.5, 1.0).l[44_100..]);
    assert!(tail(1.0) > tail(0.2) * 2.0);
    assert!(tail_seconds(1.0) > tail_seconds(0.0));
}

/// The tail covers the room: where it ends, a burst through the largest
/// room has fallen 60 dB below its own peak — the full-size room the old
/// three-second tail cut while it was still ringing at −25 dB (#1123).
#[test]
fn the_tail_ends_where_the_room_has_stopped_ringing() {
    for size in [0.0, 0.5, 0.85, 1.0] {
        let tail = (tail_seconds(size) * 44_100.0) as usize;
        let burst = 4410;
        let mut mono = vec![0.0; burst + tail];
        for (i, sample) in mono.iter_mut().take(burst).enumerate() {
            *sample = (std::f32::consts::TAU * 110.0 * i as f32 / 44_100.0).sin();
        }
        let mut buf = Stereo::centred(mono);
        apply(&mut buf, size, 0.5, 0.0, 1.0, 44_100.0);
        let loudest = |s: &[f32]| s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let (peak, end) = (loudest(&buf.l), loudest(&buf.l[buf.l.len() - 2205..]));
        let down = 20.0 * (end / peak).log10();
        assert!(down < -55.0, "size {size}: still {down:.1} dB at the end");
    }
    assert!(tail_seconds(1.0) > 10.0, "a full-size room is a long one");
}

#[test]
fn damping_dulls_the_tail() {
    let roughness = |damp: f32| {
        rung(44_100, 0.8, damp, 1.0).l[22_050..]
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .sum::<f32>()
    };
    assert!(roughness(1.0) < roughness(0.0), "damping removes highs");
}

#[test]
fn a_dry_mix_leaves_the_signal_alone() {
    assert_eq!(rung(4096, 0.7, 0.5, 0.0), impulse(4096));
}

/// `mix` blends the tail **into** the dry signal rather than out of it.
///
/// Every other test here reads a magnitude — an energy, a peak, a
/// difference between neighbours — and a room subtracted from its source
/// measures exactly like a room added to it. This is the one assertion
/// that has an opinion about which way up the tail arrives, and it asks it
/// the way the waveshaper's does: the half-wet signal has to be the
/// average of the dry one and the fully wet one, sample for sample.
#[test]
fn the_tail_is_blended_into_the_dry_signal_and_not_out_of_it() {
    let dry = impulse(8192);
    let wet = rung(8192, 0.8, 0.5, 1.0);
    let half = rung(8192, 0.8, 0.5, 0.5);
    let mut compared = 0;
    for ((h, d), w) in half.l.iter().zip(&dry.l).zip(&wet.l) {
        assert!((h - (d + w) * 0.5).abs() < 1e-6, "half of the way across");
        if w.abs() > 1e-4 {
            compared += 1;
        }
    }
    assert!(compared > 100, "only {compared} samples carried any tail");
}

/// A render rate of nothing is a nonsense a caller can hand in, and the
/// answer is the tuning's own rate rather than a rescale by zero — which
/// would collapse all twelve delay lines to a single sample and turn the
/// room into a buzz.
#[test]
fn a_rate_of_nothing_falls_back_to_the_rate_the_tunings_were_chosen_at() {
    let rung_at = |rate: f32| {
        let mut buf = impulse(8192);
        apply(&mut buf, 0.8, 0.5, 0.0, 1.0, rate);
        buf
    };
    assert_eq!(rung_at(0.0), rung_at(TUNED_RATE));
    assert!(energy(&rung_at(0.0).l) > 0.5, "and it is still a room");
    // And a rate that *is* a rate rescales, so the fallback is a fallback
    // rather than the only path: at twice the tuning rate every delay line
    // is twice as long, which is a different room.
    assert_ne!(rung_at(TUNED_RATE * 2.0), rung_at(TUNED_RATE));
}

#[test]
fn silence_in_is_silence_out_and_stays_finite() {
    let mut buf = Stereo::silence(4096);
    apply(&mut buf, 0.9, 0.2, 0.0, 1.0, 44_100.0);
    assert!(buf.l.iter().chain(&buf.r).all(|s| *s == 0.0));
    let mut buf = impulse(4096);
    apply(&mut buf, 2.0, -1.0, 0.0, 3.0, 0.0);
    assert!(
        buf.l.iter().chain(&buf.r).all(|s| s.is_finite()),
        "clamped, not exploded"
    );
}
