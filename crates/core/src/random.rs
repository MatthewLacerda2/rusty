//! SplitMix64: a pure function of its seed, never seeded from the wall clock. Every Play
//! session restarts from [`DEFAULT_SEED`]; only `Random.SetSeed` moves it.

use glam::{Vec2, Vec3};

pub const DEFAULT_SEED: u64 = 0x5EED_0000_0000_0443;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Random {
    state: u64,
}

impl Default for Random {
    fn default() -> Self {
        Self::new(DEFAULT_SEED)
    }
}

impl Random {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn set_seed(&mut self, seed: u64) {
        self.state = seed;
    }

    pub fn state(&self) -> u64 {
        self.state
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn value(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform float in `[min, max)`; `min` when the bounds are equal. Reversed
    /// bounds are accepted and interpolate the same way (Unity's behaviour).
    pub fn range_f64(&mut self, min: f64, max: f64) -> f64 {
        min + (max - min) * self.value()
    }

    /// Uniform integer in `[min, max)` — **max exclusive**, Unity's
    /// `Random.Range(int, int)`. Returns `min` when `max <= min`.
    pub fn range_i64(&mut self, min: i64, max: i64) -> i64 {
        if max <= min {
            return min;
        }
        let span = (max as i128 - min as i128) as u128;
        // Multiply-shift maps the 64-bit draw onto the span without a modulo loop;
        // the bias is at most span / 2^64, invisible for any gameplay range.
        let offset = (u128::from(self.next_u64()) * span) >> 64;
        (min as i128 + offset as i128) as i64
    }

    pub fn inside_unit_sphere(&mut self) -> Vec3 {
        loop {
            let p = Vec3::new(self.signed(), self.signed(), self.signed());
            if p.length_squared() <= 1.0 {
                return p;
            }
        }
    }

    pub fn on_unit_sphere(&mut self) -> Vec3 {
        // Archimedes: uniform z and azimuth land uniformly on the sphere.
        let z = self.signed();
        let phi = self.value() as f32 * std::f32::consts::TAU;
        let r = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(r * phi.cos(), r * phi.sin(), z)
    }

    pub fn inside_unit_circle(&mut self) -> Vec2 {
        loop {
            let p = Vec2::new(self.signed(), self.signed());
            if p.length_squared() <= 1.0 {
                return p;
            }
        }
    }

    fn signed(&mut self) -> f32 {
        (self.value() * 2.0 - 1.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream_and_set_seed_restarts_it() {
        let mut a = Random::new(7);
        let mut b = Random::new(7);
        let first: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        assert_eq!(first, (0..8).map(|_| b.next_u64()).collect::<Vec<_>>());
        a.set_seed(7);
        assert_eq!(a.next_u64(), first[0]);
        assert_ne!(Random::new(8).next_u64(), first[0]);
    }

    #[test]
    fn value_and_float_range_stay_in_bounds() {
        let mut r = Random::default();
        for _ in 0..10_000 {
            let v = r.value();
            assert!((0.0..1.0).contains(&v));
            let f = r.range_f64(-2.0, 3.0);
            assert!((-2.0..3.0).contains(&f));
        }
        assert_eq!(r.range_f64(4.0, 4.0), 4.0);
    }

    #[test]
    fn integer_range_is_max_exclusive_and_hits_every_value() {
        let mut r = Random::default();
        let mut seen = [false; 6];
        for _ in 0..10_000 {
            let n = r.range_i64(1, 7);
            assert!((1..7).contains(&n));
            seen[(n - 1) as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "every face of a d6 must come up");
        assert_eq!(r.range_i64(5, 5), 5);
        assert_eq!(r.range_i64(5, 2), 5);
        let wide = r.range_i64(i64::MIN, i64::MAX);
        assert!(wide < i64::MAX);
    }

    #[test]
    fn geometric_samples_land_where_they_claim() {
        let mut r = Random::default();
        for _ in 0..2_000 {
            assert!(r.inside_unit_sphere().length() <= 1.0 + 1e-6);
            assert!((r.on_unit_sphere().length() - 1.0).abs() < 1e-5);
            assert!(r.inside_unit_circle().length() <= 1.0 + 1e-6);
        }
    }
}
