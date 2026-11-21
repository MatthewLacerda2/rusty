//! The room, by the sample: its two sides, its tail, and what it does at each extreme of its knobs.

use super::*;

mod predelay;

fn impulse(n: usize) -> Stereo {
    let mut mono = vec![0.0; n];
    mono[0] = 1.0;
    Stereo::centred(mono)
}

fn energy(buf: &[f32]) -> f32 {
    buf.iter().map(|s| s.abs()).sum()
}

fn rung(n: usize, size: f32, damp: f32, mix: f32) -> Stereo {
    let mut buf = impulse(n);
    apply(&mut buf, size, damp, 0.0, mix, 44_100.0);
    buf
}

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

#[test]
fn the_two_sides_of_the_room_are_not_the_same_tail() {
    let buf = rung(44_100, 0.8, 0.5, 1.0);
    assert_ne!(buf.l, buf.r);
    let (left, right) = (energy(&buf.l[4410..]), energy(&buf.r[4410..]));
    assert!(left > 0.5 && right > 0.5, "{left} / {right}");
    assert!(
        (left - right).abs() < left.max(right) * 0.5,
        "one side is far louder: {left} / {right}"
    );
}

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

#[test]
fn a_rate_of_nothing_falls_back_to_the_rate_the_tunings_were_chosen_at() {
    let rung_at = |rate: f32| {
        let mut buf = impulse(8192);
        apply(&mut buf, 0.8, 0.5, 0.0, 1.0, rate);
        buf
    };
    assert_eq!(rung_at(0.0), rung_at(TUNED_RATE));
    assert!(energy(&rung_at(0.0).l) > 0.5, "and it is still a room");
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
