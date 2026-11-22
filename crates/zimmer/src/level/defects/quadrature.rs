//! A signal's envelope, without its waveform's ripple.

// Olli Niemitalo's 90-degree all-pass pair, squared: within 2 degrees of quadrature from 20 Hz
// to 20 kHz at 48 kHz.
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

#[derive(Debug, Clone, Default)]
pub(super) struct Quadrature {
    one: [Section; 4],
    other: [Section; 4],
    /// The pair reads the first chain one sample late.
    late: f64,
}

impl Quadrature {
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
