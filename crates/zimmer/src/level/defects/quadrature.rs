//! A signal's envelope, without its waveform's ripple.
//!
//! The mean square of a few milliseconds of a tone rises and falls twice a
//! cycle with where the window happens to cut the waveform, and a low tone's
//! rise and fall is as large as a gain step: measured that way, a steady
//! 155 Hz tone read as a staircase. Two all-pass chains whose outputs stay a
//! quarter cycle apart across the audible range turn the signal into a pair
//! whose summed squares are a sine's amplitude whatever its phase, so a steady
//! tone or chord reads flat and only a real change in level moves it.
//!
//! The coefficients are Olli Niemitalo's published 90° all-pass pair: within
//! two degrees of quadrature from 20 Hz to 20 kHz at 48 kHz.

/// The two chains' coefficients, already squared.
const ONE: [f64; 4] = [
    0.479_400_865_6,
    0.876_218_493_5,
    0.976_597_589_5,
    0.997_499_255_9,
];
const OTHER: [f64; 4] = [
    0.161_758_498_4,
    0.733_028_932_3,
    0.945_349_700_3,
    0.990_599_156_7,
];

/// One second-order all-pass section: `y[n] = c·(x[n] + y[n−2]) − x[n−2]`.
#[derive(Debug, Clone, Copy, Default)]
struct Section {
    x: [f64; 2],
    y: [f64; 2],
}

impl Section {
    fn run(&mut self, coefficient: f64, x: f64) -> f64 {
        let y = coefficient * (x + self.y[1]) - self.x[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// The pair of chains for one channel.
#[derive(Debug, Clone, Default)]
pub(super) struct Quadrature {
    one: [Section; 4],
    other: [Section; 4],
    /// The first chain's previous output: it is read a sample late.
    late: f64,
}

impl Quadrature {
    /// The squared envelope of the signal at this sample, halved so a steady
    /// signal reads as its own mean square.
    pub(super) fn energy(&mut self, sample: f32) -> f64 {
        let x = f64::from(sample);
        let one = self
            .one
            .iter_mut()
            .zip(ONE)
            .fold(x, |x, (s, c)| s.run(c, x));
        let other = self
            .other
            .iter_mut()
            .zip(OTHER)
            .fold(x, |x, (s, c)| s.run(c, x));
        let late = std::mem::replace(&mut self.late, one);
        (late * late + other * other) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::Quadrature;

    /// The whole reason for the pair: a steady tone's envelope is flat, low
    /// or high, where its square rises and falls twice a cycle.
    #[test]
    fn a_steady_tone_has_a_flat_envelope_across_the_audible_range() {
        for hz in [20.0, 60.0, 155.0, 1_000.0, 15_000.0, 20_000.0] {
            let mut pair = Quadrature::default();
            let energies: Vec<f64> = (0..96_000)
                .map(|i| {
                    let t = f64::from(i) / 48_000.0;
                    pair.energy((0.5 * (std::f64::consts::TAU * hz * t).sin()) as f32)
                })
                .skip(48_000)
                .collect();
            let (low, high) = energies.iter().fold((f64::MAX, 0.0_f64), |(low, high), e| {
                (low.min(*e), high.max(*e))
            });
            let ripple_db = 10.0 * (high / low).log10();
            assert!(ripple_db < 0.6, "{hz} Hz ripples {ripple_db:.2} dB");
            assert!(
                (high / 0.125 - 1.0).abs() < 0.05,
                "{hz} Hz reads {high}, not its mean square"
            );
        }
    }
}
