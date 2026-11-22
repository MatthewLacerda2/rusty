//! One delay line, read at a fractional position.

/// `far` is one sample further back, so `frac` weights backwards in time, the way a delay grows.
pub(super) fn read(line: &[f32], write: usize, delay: f32) -> f32 {
    let len = line.len();
    let whole = delay.floor();
    let frac = delay - whole;
    // At least one back and two short of a lap: both taps stay off the frame being written.
    let back = (whole as usize).clamp(1, len - 2);
    let near = (write + len - back) % len;
    let far = (near + len - 1) % len;
    line[near] * (1.0 - frac) + line[far] * frac
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_interpolated_read_is_the_weighted_average_of_its_two_neighbours() {
        let line: Vec<f32> = (0..16).map(|i| i as f32).collect();
        assert_eq!(read(&line, 15, 4.0), 11.0, "four whole samples back");
        assert_eq!(read(&line, 15, 4.5), 10.5, "half a sample further back");
        assert_eq!(read(&line, 15, 4.25), 10.75, "and a quarter of the way");
        assert_eq!(read(&line, 1, 4.0), 13.0, "the ring wraps, it does not end");
        assert_eq!(read(&line, 15, 0.0), 14.0);
        assert_eq!(read(&line, 15, 500.0), read(&line, 15, 14.0));
    }

    #[test]
    fn the_modulated_read_glides_rather_than_stepping() {
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
