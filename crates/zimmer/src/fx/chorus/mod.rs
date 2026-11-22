pub(crate) mod line;
pub(crate) mod voices;

use std::f32::consts::TAU;

use crate::stereo::Stereo;
use line::read;
use voices::{ensemble, MAX_VOICES, MIN_VOICES};

/// Under ~10 ms the comb notches colour the tone; over ~30 ms the copies slap back.
const BASE_SECONDS: f32 = 0.015;

const SWEEP_SECONDS: f32 = 0.010;

/// Hz; faster, the sidebands read as inharmonic tones, not a detune.
const MAX_RATE: f32 = 10.0;

pub(crate) fn apply(
    buf: &mut Stereo,
    lfo_rate: f32,
    depth: f32,
    voices: usize,
    mix: f32,
    rate: f32,
) {
    // Only a bad `rate` returns early (at zero the two-sample line panics `read`'s clamp); the
    // other inputs are clamped.
    if !rate.is_finite() || rate <= 0.0 {
        return;
    }
    let mix = clamp_unit(mix);
    let depth = clamp_unit(depth);
    let lfo_rate = if lfo_rate.is_finite() {
        lfo_rate.clamp(0.0, MAX_RATE)
    } else {
        0.0
    };
    let mut ensemble = ensemble(voices.clamp(MIN_VOICES, MAX_VOICES), lfo_rate, rate);

    let len = line_len(depth, rate);
    let mut line = vec![0.0f32; len];
    let centre = BASE_SECONDS * rate;
    let sweep = depth * SWEEP_SECONDS * rate;

    for i in 0..buf.frames() {
        let (dry_l, dry_r) = (buf.l[i], buf.r[i]);
        line[i % len] = (dry_l + dry_r) * 0.5;
        let (mut wet_l, mut wet_r) = (0.0, 0.0);
        for voice in &mut ensemble {
            let tap = read(&line, i, centre + sweep * voice.phase.sin());
            wet_l += tap * voice.gains.0;
            wet_r += tap * voice.gains.1;
            voice.phase = advance(voice.phase, voice.step);
        }
        buf.l[i] = dry_l * (1.0 - mix) + wet_l * mix;
        buf.r[i] = dry_r * (1.0 - mix) + wet_r * mix;
    }
}

/// Two past the deepest read, so the far tap never reaches the frame being written.
fn line_len(depth: f32, rate: f32) -> usize {
    (tail_seconds(depth) * rate).ceil() as usize + 2
}

/// Wrapped, so the phase keeps its `f32` precision over a long buffer.
fn advance(phase: f32, step: f32) -> f32 {
    (phase + step) % TAU
}

pub(crate) fn tail_seconds(depth: f32) -> f32 {
    BASE_SECONDS + clamp_unit(depth) * SWEEP_SECONDS
}

fn clamp_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 44_100.0;

    fn tone(freq: f32) -> Stereo {
        Stereo::centred(
            (0..RATE as usize)
                .map(|i| (TAU * freq * i as f32 / RATE).sin())
                .collect(),
        )
    }

    fn impulse(frames: usize) -> Stereo {
        let mut mono = vec![0.0; frames];
        mono[0] = 1.0;
        Stereo::centred(mono)
    }

    fn arrivals(buf: &Stereo) -> Vec<usize> {
        (0..buf.frames())
            .filter(|i| buf.l[*i].abs() + buf.r[*i].abs() > 1e-3)
            .collect()
    }

    fn correlation(buf: &Stereo) -> f32 {
        let dot: f32 = buf.l.iter().zip(&buf.r).map(|(l, r)| l * r).sum();
        let energy = |c: &[f32]| c.iter().map(|s| s * s).sum::<f32>().sqrt();
        dot / (energy(&buf.l) * energy(&buf.r)).max(f32::MIN_POSITIVE)
    }

    #[test]
    fn a_centred_source_comes_back_wide() {
        let dry = tone(440.0);
        assert!((correlation(&dry) - 1.0).abs() < 1e-4, "it went in centred");
        let mut wet = dry.clone();
        apply(&mut wet, 0.6, 0.8, 4, 1.0, RATE);
        assert_ne!(wet.l, wet.r, "the sides disagree");
        let widened = correlation(&wet);
        assert!(widened < 0.9, "the sides are still {widened} alike");
    }

    #[test]
    fn the_same_settings_render_the_same_samples() {
        let mut once = tone(220.0);
        let mut again = tone(220.0);
        apply(&mut once, 1.3, 0.7, 3, 0.6, RATE);
        apply(&mut again, 1.3, 0.7, 3, 0.6, RATE);
        assert_eq!(once, again);
    }

    #[test]
    fn a_dry_mix_and_a_degenerate_setting_change_nothing() {
        let original = tone(330.0);
        let mut dry = original.clone();
        apply(&mut dry, 0.5, 0.5, 3, 0.0, RATE);
        assert_eq!(dry, original, "a dry mix is not an approximation");
        for (lfo_rate, depth, rate) in [
            (f32::NAN, 0.5, RATE),
            (0.5, f32::INFINITY, RATE),
            (0.5, 0.5, 0.0),
            (1e9, 2.0, RATE),
        ] {
            let mut buf = original.clone();
            apply(&mut buf, lfo_rate, depth, 3, 1.0, rate);
            assert!(buf.l.iter().chain(&buf.r).all(|s| s.is_finite()));
        }
    }

    #[test]
    fn a_copy_arrives_exactly_the_centre_delay_behind_the_dry_signal() {
        let mut buf = impulse(4096);
        apply(&mut buf, 0.0, 0.0, 4, 1.0, RATE);
        // 661.5 samples: only an interpolated read splits the impulse across two taps.
        let at = BASE_SECONDS * RATE;
        assert!(
            (at.fract() - 0.5).abs() < 1e-3,
            "the fixture rests on a half"
        );
        let near = at.floor() as usize;
        for side in [&buf.l, &buf.r] {
            assert!((side[near] - 0.5).abs() < 1e-4, "near tap {}", side[near]);
            assert!(
                (side[near + 1] - 0.5).abs() < 1e-4,
                "far tap {}",
                side[near + 1]
            );
            assert_eq!(side[near - 1], 0.0, "and silence either side of the pair");
            assert_eq!(side[near + 2], 0.0);
            assert_eq!(side[0], 0.0, "fully wet keeps no dry signal");
        }
    }

    #[test]
    fn depth_places_each_voice_at_its_own_delay_inside_the_documented_range() {
        let mut buf = impulse(4096);
        apply(&mut buf, 0.0, 1.0, 4, 1.0, RATE);
        let (centre, sweep) = (BASE_SECONDS * RATE, SWEEP_SECONDS * RATE);
        let landed = arrivals(&buf);
        for voice in ensemble(4, 0.0, RATE) {
            let at = (centre + sweep * voice.phase.sin()).floor() as usize;
            assert!(
                landed.contains(&at) || landed.contains(&(at + 1)),
                "no copy at {at}; energy is at {landed:?}"
            );
        }
        let (first, last) = (landed[0] as f32, landed[landed.len() - 1] as f32);
        assert!(
            first >= centre - sweep - 1.0,
            "nothing arrives before the range"
        );
        assert!(last <= centre + sweep + 1.0, "nor after it");
        assert!(
            last - first > sweep,
            "and they are spread across most of it"
        );

        // Two voices: the left is the first alone, pinning the sweep's sign.
        let mut pair = impulse(4096);
        apply(&mut pair, 0.0, 1.0, 2, 1.0, RATE);
        let alone = &ensemble(2, 0.0, RATE)[0];
        let at = (centre + sweep * alone.phase.sin()).floor() as usize;
        let caught = pair.l[at] + pair.l[at + 1];
        assert!(caught > 0.9, "the copy is not at {at}: {caught} of it is");
    }

    #[test]
    fn a_copy_sweeps_through_its_delays_and_is_back_after_one_of_its_cycles() {
        let hz = 4.0;
        let period = (TAU / ensemble(2, hz, RATE)[0].step).round() as usize;
        let mut buf = Stereo::silence(period + 4096);
        for probe in [0, period / 4, period / 2, period] {
            buf.l[probe] = 1.0;
            buf.r[probe] = 1.0;
        }
        apply(&mut buf, hz, 1.0, 2, 1.0, RATE);
        let loudest = |from: usize| {
            (from..from + 2048)
                .max_by(|a, b| buf.l[*a].abs().total_cmp(&buf.l[*b].abs()))
                .expect("a window to search")
                - from
        };
        let (start, middle, end) = (loudest(0), loudest(period / 2), loudest(period));
        assert!(
            start.abs_diff(end) <= 4,
            "a cycle on, back where it began: {start} then {end}"
        );
        assert!(
            start.abs_diff(middle) > 100,
            "and half a cycle on, somewhere else: {start} then {middle}"
        );

        let quarter = loudest(period / 4);
        assert!(
            quarter.abs_diff(start) > 50 && quarter.abs_diff(middle) > 50,
            "a quarter cycle is its own place: {start}, {quarter}, {middle}"
        );
    }

    #[test]
    fn mix_blends_wet_against_dry() {
        let dry = tone(220.0);
        let mut wet = dry.clone();
        apply(&mut wet, 0.7, 0.6, 3, 1.0, RATE);
        let mut half = dry.clone();
        apply(&mut half, 0.7, 0.6, 3, 0.5, RATE);
        for i in 0..dry.frames() {
            let expected = (dry.l[i] + wet.l[i]) * 0.5;
            assert!((half.l[i] - expected).abs() < 1e-5, "left at {i}");
            let expected = (dry.r[i] + wet.r[i]) * 0.5;
            assert!((half.r[i] - expected).abs() < 1e-5, "right at {i}");
        }
    }

    #[test]
    fn the_tail_is_the_deepest_read_and_nothing_more() {
        assert!((tail_seconds(0.0) - BASE_SECONDS).abs() < 1e-9);
        assert!((tail_seconds(1.0) - (BASE_SECONDS + SWEEP_SECONDS)).abs() < 1e-9);
        assert_eq!(tail_seconds(9.0), tail_seconds(1.0), "clamped");
        assert_eq!(tail_seconds(f32::NAN), tail_seconds(0.0));
        assert!(tail_seconds(1.0) < 0.03, "a chorus is not a room");
    }
}
