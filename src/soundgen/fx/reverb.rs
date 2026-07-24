//! src/soundgen/fx/reverb.rs — Freeverb (#357).
//!
//! Jezar's **Freeverb** is the public-domain reverb everyone reaches for at this
//! scale: eight parallel damped comb filters (the dense early build-up) feeding four
//! series allpass filters (the diffusion that smears them into a tail). Around a
//! hundred lines, no tuning tables to invent, and it has sounded good in trackers and
//! game audio for twenty-five years — exactly the "standard, proven algorithm"
//! bar this layer is held to.
//!
//! The comb/allpass lengths are the original prime-ish tunings, chosen at 44.1 kHz
//! and rescaled if a bake ever runs at another rate. `size` sets the comb feedback
//! (room size), `damp` how fast the tail loses its highs, `mix` the wet/dry blend.

/// The eight parallel comb-filter lengths, in samples at 44.1 kHz.
const COMB_LENGTHS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
/// The four series allpass lengths, in samples at 44.1 kHz.
const ALLPASS_LENGTHS: [usize; 4] = [556, 441, 341, 225];
/// The rate the tunings above were chosen at.
const TUNED_RATE: f32 = 44_100.0;
/// Input scaling, so eight summed combs stay in range.
const FIXED_GAIN: f32 = 0.015;
/// `size` maps to comb feedback as `size × SCALE + OFFSET`.
const ROOM_SCALE: f32 = 0.28;
const ROOM_OFFSET: f32 = 0.7;
/// `damp` maps to the comb's internal lowpass coefficient by this scale.
const DAMP_SCALE: f32 = 0.4;
/// The allpass feedback Freeverb fixes at 0.5.
const ALLPASS_FEEDBACK: f32 = 0.5;

/// One damped comb filter: a delay line with a one-pole lowpass in its feedback
/// path, which is what makes the tail darken as it decays.
struct Comb {
    line: Vec<f32>,
    index: usize,
    store: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            line: vec![0.0; len.max(1)],
            index: 0,
            store: 0.0,
        }
    }

    fn step(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let output = self.line[self.index];
        self.store = output * (1.0 - damp) + self.store * damp;
        self.line[self.index] = input + self.store * feedback;
        self.index = (self.index + 1) % self.line.len();
        output
    }
}

/// One allpass filter: passes every frequency at equal level but scrambles their
/// phases, turning the combs' discrete echoes into a smooth tail.
struct Allpass {
    line: Vec<f32>,
    index: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            line: vec![0.0; len.max(1)],
            index: 0,
        }
    }

    fn step(&mut self, input: f32) -> f32 {
        let buffered = self.line[self.index];
        self.line[self.index] = input + buffered * ALLPASS_FEEDBACK;
        self.index = (self.index + 1) % self.line.len();
        buffered - input
    }
}

/// Apply Freeverb to `buf` in place.
pub fn apply(buf: &mut [f32], size: f32, damp: f32, mix: f32, rate: f32) {
    let scale = if rate > 0.0 { rate / TUNED_RATE } else { 1.0 };
    let scaled = |len: usize| ((len as f32 * scale).round() as usize).max(1);
    let mut combs: Vec<Comb> = COMB_LENGTHS.iter().map(|l| Comb::new(scaled(*l))).collect();
    let mut allpasses: Vec<Allpass> = ALLPASS_LENGTHS
        .iter()
        .map(|l| Allpass::new(scaled(*l)))
        .collect();

    let feedback = size.clamp(0.0, 1.0) * ROOM_SCALE + ROOM_OFFSET;
    let damp = damp.clamp(0.0, 1.0) * DAMP_SCALE;
    let mix = mix.clamp(0.0, 1.0);
    for s in buf.iter_mut() {
        let input = *s * FIXED_GAIN;
        let mut wet: f32 = combs
            .iter_mut()
            .map(|c| c.step(input, feedback, damp))
            .sum();
        for ap in allpasses.iter_mut() {
            wet = ap.step(wet);
        }
        *s = *s * (1.0 - mix) + wet * mix;
    }
}

/// How long the tail rings after the dry signal stops, in seconds — what the
/// renderer pads the buffer by so the reverb is not cut off.
pub fn tail_seconds(size: f32) -> f32 {
    0.5 + size.clamp(0.0, 1.0) * 2.5
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A full-scale impulse then silence — the classic reverb probe.
    fn impulse(n: usize) -> Vec<f32> {
        let mut buf = vec![0.0; n];
        buf[0] = 1.0;
        buf
    }

    /// Total energy in a window, the measure a tail is judged by.
    fn energy(buf: &[f32]) -> f32 {
        buf.iter().map(|s| s.abs()).sum()
    }

    #[test]
    fn an_impulse_becomes_a_decaying_tail() {
        let mut buf = impulse(88_200);
        apply(&mut buf, 0.8, 0.5, 1.0, 44_100.0);
        let early = energy(&buf[..22_050]);
        let late = energy(&buf[66_150..]);
        assert!(early > 0.5, "the reverb produced almost nothing ({early})");
        assert!(late < early, "the tail must decay: {early} → {late}");
        assert!(buf.iter().all(|s| s.abs() <= 1.0), "and never clip");
    }

    #[test]
    fn a_bigger_room_rings_longer() {
        let tail = |size: f32| {
            let mut buf = impulse(88_200);
            apply(&mut buf, size, 0.5, 1.0, 44_100.0);
            energy(&buf[44_100..])
        };
        assert!(tail(1.0) > tail(0.2) * 2.0);
        assert!(tail_seconds(1.0) > tail_seconds(0.0));
    }

    #[test]
    fn damping_dulls_the_tail() {
        let roughness = |damp: f32| {
            let mut buf = impulse(44_100);
            apply(&mut buf, 0.8, damp, 1.0, 44_100.0);
            buf[22_050..]
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .sum::<f32>()
        };
        assert!(roughness(1.0) < roughness(0.0), "damping removes highs");
    }

    #[test]
    fn a_dry_mix_leaves_the_signal_alone() {
        let mut buf = impulse(4096);
        apply(&mut buf, 0.7, 0.5, 0.0, 44_100.0);
        assert_eq!(buf, impulse(4096));
    }

    #[test]
    fn silence_in_is_silence_out_and_stays_finite() {
        let mut buf = vec![0.0; 4096];
        apply(&mut buf, 0.9, 0.2, 1.0, 44_100.0);
        assert!(buf.iter().all(|s| *s == 0.0));
        let mut buf = impulse(4096);
        apply(&mut buf, 2.0, -1.0, 3.0, 0.0);
        assert!(buf.iter().all(|s| s.is_finite()), "clamped, not exploded");
    }
}
