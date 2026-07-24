//! src/soundgen/core/fm.rs — two-operator FM (#357).
//!
//! One sine (the *modulator*) bends the frequency of another (the *carrier*). That
//! is the whole algorithm, and it buys the timbres subtractive synthesis is worst
//! at: electric pianos, bells, glassy plucks and metallic impacts.
//!
//! - `ratio` sets the modulator's frequency as a multiple of the played pitch.
//!   Whole-number ratios stay harmonic (tonal); ratios like `1.41` go inharmonic —
//!   that is where bells and hits live.
//! - `index` is the modulation depth: how many sidebands, i.e. how bright.
//! - `mod_decay` is the modulator's *own* exponential decay, in seconds. It is what
//!   makes an FM note read as *struck*: a bright transient collapsing to a near-sine
//!   body, the same way a real hammer or mallet behaves.
//!
//! Deliberately two operators, sines only. Six-operator FM is a synth in its own
//! right; this is the one extra source that pays for itself.

use std::f32::consts::TAU;

/// Render a two-operator FM note into `out`, following the per-sample frequency
/// track `freqs` (so an LFO vibrato applies to carrier and modulator alike).
pub fn render(out: &mut [f32], freqs: &[f32], ratio: f32, index: f32, decay: f32, rate: f32) {
    let mut carrier = 0.0f32;
    let mut modulator = 0.0f32;
    for (i, s) in out.iter_mut().enumerate() {
        let base = freqs.get(i).copied().unwrap_or(0.0);
        let env = mod_envelope(i as f32 / rate, decay);
        *s = (TAU * carrier + index * env * (TAU * modulator).sin()).sin();
        carrier = (carrier + base / rate).fract();
        modulator = (modulator + base * ratio / rate).fract();
    }
}

/// The modulator's exponential decay at time `t`. A non-positive `mod_decay` means
/// "no bright transient at all", i.e. the carrier stays a plain sine.
#[inline]
fn mod_envelope(t: f32, decay: f32) -> f32 {
    if decay <= 0.0 {
        return 0.0;
    }
    (-t / decay).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_note(ratio: f32, index: f32, decay: f32, n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n];
        render(&mut out, &vec![220.0; n], ratio, index, decay, 44_100.0);
        out
    }

    /// Sum of absolute sample-to-sample change — a cheap brightness proxy.
    fn roughness(buf: &[f32]) -> f32 {
        buf.windows(2).map(|w| (w[1] - w[0]).abs()).sum()
    }

    #[test]
    fn output_stays_inside_unity() {
        let buf = render_note(2.0, 8.0, 0.3, 44_100);
        assert!(buf.iter().all(|s| s.abs() <= 1.0 + 1e-6));
        assert!(buf.iter().any(|s| s.abs() > 0.5), "and is not silence");
    }

    #[test]
    fn a_higher_index_makes_a_brighter_tone() {
        let quiet = render_note(2.0, 0.5, 10.0, 4410);
        let bright = render_note(2.0, 8.0, 10.0, 4410);
        assert!(roughness(&bright) > roughness(&quiet) * 2.0);
    }

    #[test]
    fn zero_index_or_no_decay_collapses_to_a_plain_sine() {
        let plain = render_note(2.0, 0.0, 0.3, 4410);
        let no_decay = render_note(2.0, 8.0, 0.0, 4410);
        for (a, b) in plain.iter().zip(&no_decay) {
            assert!((a - b).abs() < 1e-6, "both must be the bare carrier");
        }
        let rising = plain
            .windows(2)
            .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
            .count();
        assert_eq!(rising, 22, "4410 samples of a 220 Hz sine");
    }

    #[test]
    fn the_modulator_decays_so_the_note_dulls_as_it_rings() {
        let buf = render_note(3.0, 10.0, 0.05, 22_050);
        let attack = roughness(&buf[..2205]);
        let tail = roughness(&buf[19_845..]);
        assert!(
            attack > tail * 2.0,
            "attack {attack} should out-shine tail {tail}"
        );
        assert!((mod_envelope(0.0, 0.05) - 1.0).abs() < 1e-6);
        assert!(mod_envelope(0.5, 0.05) < 0.001);
    }
}
