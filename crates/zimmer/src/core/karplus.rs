//! Karplus-Strong plucked string.

use super::noise;

/// Samples: a shorter line is no string, only noise.
const MIN_DELAY: usize = 2;
/// Hash channel ("KS") the pluck draws on, so it never mirrors a `noise` source.
const EXCITATION_CHANNEL: u64 = 0x4b53;

pub(crate) fn render(
    out: &mut [f32],
    freq: f32,
    damping: f32,
    brightness: f32,
    seed: u64,
    rate: f32,
) {
    let len = delay_len(freq, rate);
    let mut line = excitation(len, brightness.clamp(0.0, 1.0), seed);
    // Half: each round averages two samples, the string losing its highs.
    let feedback = damping.clamp(0.0, 1.0) * 0.5;
    let mut read = 0usize;
    let mut previous = 0.0f32;
    for s in out.iter_mut() {
        let current = line[read];
        *s = current;
        line[read] = feedback * (current + previous);
        previous = current;
        read = (read + 1) % len;
    }
}

fn delay_len(freq: f32, rate: f32) -> usize {
    if freq <= 0.0 || !freq.is_finite() {
        return MIN_DELAY;
    }
    // Less half a sample: the feedback's two-sample average delays by that much.
    ((rate / freq - 0.5).round() as usize).max(MIN_DELAY)
}

fn excitation(len: usize, brightness: f32, seed: u64) -> Vec<f32> {
    let white: Vec<f32> = (0..len)
        .map(|i| noise::white(i as i64, EXCITATION_CHANNEL, seed))
        .collect();
    (0..len)
        .map(|i| {
            let previous = white[(i + len - 1) % len];
            let dull = 0.5 * (white[i] + previous);
            dull + brightness * (white[i] - dull)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Autocorrelation: a plucked string's harmonics cross zero several times a cycle, so counting
    /// zero crossings fails.
    fn estimated_hz(buf: &[f32], rate: f32) -> f32 {
        let best = (20..600)
            .max_by(|&a, &b| {
                let energy = |lag: usize| -> f32 {
                    buf[..buf.len() - lag]
                        .iter()
                        .zip(&buf[lag..])
                        .map(|(x, y)| x * y)
                        .sum()
                };
                energy(a).total_cmp(&energy(b))
            })
            .unwrap_or(1);
        rate / best as f32
    }

    #[test]
    fn a_pluck_rings_at_the_requested_pitch() {
        for wanted in [110.0, 220.0, 440.0] {
            let mut buf = vec![0.0; 44_100];
            render(&mut buf, wanted, 0.999, 0.5, 1, 44_100.0);
            let hz = estimated_hz(&buf[22_050..], 44_100.0);
            let cents = 1200.0 * (hz / wanted).log2();
            assert!(
                cents.abs() < 15.0,
                "rings at {hz} Hz, {cents} cents off {wanted}"
            );
        }
    }

    #[test]
    fn damping_decides_how_long_the_string_rings() {
        let energy = |damping: f32| {
            let mut buf = vec![0.0; 44_100];
            render(&mut buf, 220.0, damping, 0.5, 1, 44_100.0);
            buf[22_050..].iter().map(|s| s.abs()).sum::<f32>()
        };
        assert!(
            energy(0.999) > energy(0.9) * 10.0,
            "a damped string dies faster"
        );
        assert!(
            energy(0.5) < 1e-3,
            "heavy damping is silent by half a second"
        );
    }

    #[test]
    fn brightness_controls_the_attack_spectrum() {
        // A brighter excitation moves harder between adjacent samples at onset.
        let roughness = |brightness: f32| {
            let mut buf = vec![0.0; 2048];
            render(&mut buf, 440.0, 0.99, brightness, 3, 44_100.0);
            buf.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>()
        };
        assert!(roughness(1.0) > roughness(0.0));
    }

    #[test]
    fn the_same_seed_replays_the_same_string() {
        let (mut a, mut b) = (vec![0.0; 4096], vec![0.0; 4096]);
        render(&mut a, 330.0, 0.99, 0.7, 11, 44_100.0);
        render(&mut b, 330.0, 0.99, 0.7, 11, 44_100.0);
        assert_eq!(a, b);
        let mut c = vec![0.0; 4096];
        render(&mut c, 330.0, 0.99, 0.7, 12, 44_100.0);
        assert_ne!(a, c, "a different seed plucks differently");
    }

    #[test]
    fn a_degenerate_pitch_still_renders_finite_samples() {
        for freq in [0.0, -100.0, f32::INFINITY, 40_000.0] {
            let mut buf = vec![0.0; 512];
            render(&mut buf, freq, 0.99, 0.5, 1, 44_100.0);
            assert!(buf.iter().all(|s| s.is_finite()), "freq {freq}");
        }
    }
}
