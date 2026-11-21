//! One delay line, read at a fractional position.
//!
//! A ring buffer and a reader, and the reader is the whole reason this is its
//! own module: a modulated delay read at integer sample positions steps
//! between whole samples, and every step is a discontinuity. What should be a
//! smooth glide in pitch arrives as a staircase of them — zipper noise riding
//! on the effect. Linear interpolation between the two samples either side of
//! the fractional position is the minimum that makes a chorus a chorus, and it
//! is the detail [`super`]'s doc calls out for exactly that reason.

/// The line read `delay` samples behind write position `write`, linearly
/// interpolated between the two samples either side of that position.
///
/// `near` is the whole-sample part and `far` is one sample further back, so
/// the fraction weights *backwards* in time — the direction the delay grows.
pub(super) fn read(line: &[f32], write: usize, delay: f32) -> f32 {
    let len = line.len();
    let whole = delay.floor();
    let frac = delay - whole;
    let back = (whole as usize).clamp(1, len - 2);
    let near = (write + len - back) % len;
    let far = (near + len - 1) % len;
    line[near] * (1.0 - frac) + line[far] * frac
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The interpolator, pinned to values rather than to smoothness. A read
    /// at a whole sample is that sample; a read between two is their weighted
    /// average, weighted *backwards* in time — the direction a delay grows.
    #[test]
    fn an_interpolated_read_is_the_weighted_average_of_its_two_neighbours() {
        let line: Vec<f32> = (0..16).map(|i| i as f32).collect();
        assert_eq!(read(&line, 15, 4.0), 11.0, "four whole samples back");
        assert_eq!(read(&line, 15, 4.5), 10.5, "half a sample further back");
        assert_eq!(read(&line, 15, 4.25), 10.75, "and a quarter of the way");
        assert_eq!(read(&line, 1, 4.0), 13.0, "the ring wraps, it does not end");
        // Both clamps, which are what keep the two taps off the sample being
        // written and inside the line: one behind at the shallowest, and two
        // short of a full lap at the deepest.
        assert_eq!(read(&line, 15, 0.0), 14.0);
        assert_eq!(read(&line, 15, 500.0), read(&line, 15, 14.0));
    }

    #[test]
    fn the_modulated_read_glides_rather_than_stepping() {
        // A ramp read back at a slowly growing delay: with integer reads the
        // output would repeat samples and then jump. Linear interpolation
        // makes every consecutive difference smaller than one whole step.
        let line: Vec<f32> = (0..64).map(|i| i as f32).collect();
        let taps: Vec<f32> = (0..40)
            .map(|i| read(&line, 63, 8.0 + i as f32 * 0.05))
            .collect();
        for (a, b) in taps.iter().zip(taps.iter().skip(1)) {
            let step = (b - a).abs();
            assert!(step > 0.0 && step < 0.5, "a staircase step of {step}");
        }
    }
}
