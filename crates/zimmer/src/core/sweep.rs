//! sweep — a cutoff moved from outside the note, as the per-sample base
//! [`super::tracks::cutoff_track`] multiplies.

const SWEEP_STEP: usize = 32;

pub(crate) fn swept(sweep: &dyn Fn(usize) -> f32, n: usize) -> Vec<f32> {
    let last = n.saturating_sub(1);
    let read = |k: usize| (k * SWEEP_STEP).min(last);
    let values: Vec<f32> = (0..=last / SWEEP_STEP + 1)
        .map(|k| sweep(read(k)))
        .collect();
    (0..n)
        .map(|frame| {
            let (k, into) = (frame / SWEEP_STEP, frame % SWEEP_STEP);
            let (from, to) = (values[k], values[k + 1]);
            if into == 0 {
                return from;
            }
            from + (to - from) * into as f32 / (read(k + 1) - read(k)) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_straight_line_is_read_back_exactly_at_every_frame() {
        let line = |frame: usize| 100.0 + 2.0 * frame as f32;
        for n in [1, 2, 31, 32, 33, 100, 1000] {
            let swept = swept(&line, n);
            assert_eq!(swept.len(), n, "{n} frames asked for");
            for (frame, value) in swept.iter().enumerate() {
                assert!(
                    (value - line(frame)).abs() < 1e-3,
                    "frame {frame} of {n}: {value}"
                );
            }
        }
    }

    #[test]
    fn a_constant_is_handed_back_bit_for_bit() {
        assert!(swept(&|_| 440.0, 500).iter().all(|v| *v == 440.0));
        assert!(swept(&|_| 440.0, 0).is_empty());
    }
}
