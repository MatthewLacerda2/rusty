//! Cuts a channel to its take like Unity's LocalStart..LocalStop: a key is
//! interpolated in at each end, and times shift to start at 0 (glTF's sampler
//! input must be ≥ 0).

/// `keys` ascend by time. A range with `end <= begin` keeps one key, at 0.
pub fn clamp<T: Copy>(
    keys: &[(f64, T)],
    begin: f64,
    end: f64,
    lerp: impl Fn(T, T, f64) -> T,
) -> Vec<(f32, T)> {
    if keys.is_empty() {
        return Vec::new();
    }
    let at = |t: f64| sample(keys, t, &lerp);
    let mut out = vec![(0.0, at(begin))];
    out.extend(
        keys.iter()
            .filter(|(t, _)| *t > begin && *t < end)
            .map(|&(t, v)| ((t - begin) as f32, v)),
    );
    if end > begin {
        out.push(((end - begin) as f32, at(end)));
    }
    out
}

/// Held flat before the first key and after the last.
fn sample<T: Copy>(keys: &[(f64, T)], t: f64, lerp: &impl Fn(T, T, f64) -> T) -> T {
    let next = keys.partition_point(|(k, _)| *k <= t);
    match (next.checked_sub(1).map(|i| keys[i]), keys.get(next)) {
        (Some((_, a)), None) => a,
        (None, Some(&(_, b))) => b,
        (Some((ta, a)), Some(&(tb, b))) => lerp(a, b, (t - ta) / (tb - ta)),
        (None, None) => unreachable!("clamp never samples an empty channel"),
    }
}

#[cfg(test)]
mod tests {
    use super::clamp;

    fn cut(keys: &[(f64, f64)], begin: f64, end: f64) -> Vec<(f32, f64)> {
        clamp(keys, begin, end, |a, b, t| a + (b - a) * t)
    }

    #[test]
    fn a_take_starting_below_zero_shifts_to_zero() {
        let keys = [(-1.0, 10.0), (0.0, 20.0), (1.0, 30.0)];
        assert_eq!(
            cut(&keys, -1.0, 1.0),
            [(0.0, 10.0), (1.0, 20.0), (2.0, 30.0)]
        );
    }

    #[test]
    fn a_key_before_the_start_is_cut_and_the_start_interpolated() {
        let keys = [(0.0, 0.0), (2.0, 20.0), (3.0, 30.0)];
        assert_eq!(
            cut(&keys, 1.0, 3.0),
            [(0.0, 10.0), (1.0, 20.0), (2.0, 30.0)]
        );
    }

    #[test]
    fn a_key_past_the_stop_is_cut_and_the_stop_interpolated() {
        let keys = [(0.0, 0.0), (1.0, 10.0), (4.0, 40.0)];
        assert_eq!(cut(&keys, 0.0, 2.0), [(0.0, 0.0), (1.0, 10.0), (2.0, 20.0)]);
    }

    #[test]
    fn a_curve_shorter_than_the_take_holds_its_ends() {
        assert_eq!(cut(&[(3.0, 5.0)], 0.0, 2.0), [(0.0, 5.0), (2.0, 5.0)]);
        assert_eq!(cut(&[], 0.0, 2.0), []);
        assert_eq!(cut(&[(1.0, 5.0)], 1.0, 1.0), [(0.0, 5.0)]);
    }
}
