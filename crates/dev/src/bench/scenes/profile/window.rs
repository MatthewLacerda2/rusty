#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Window {
    pub start: u64,
    pub end: u64,
}

impl Window {
    pub fn contains(&self, t: u64) -> bool {
        (self.start..=self.end).contains(&t)
    }
}

pub fn inside(windows: &[Window], t: u64) -> bool {
    windows.iter().any(|w| w.contains(t))
}

pub fn seconds(windows: &[Window]) -> f64 {
    let ns: u64 = windows.iter().map(|w| w.end.saturating_sub(w.start)).sum();
    ns as f64 / 1e9
}

/// Nanoseconds of `CLOCK_MONOTONIC`, the clock `perf::start` passes `perf record -k`: change
/// both together.
#[cfg(target_os = "linux")]
pub fn now() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable `timespec`, which is all `clock_gettime` writes.
    let status = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    debug_assert_eq!(status, 0, "CLOCK_MONOTONIC is always readable");
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// Nanoseconds since the first call: off Linux only the windows' lengths are read.
#[cfg(not(target_os = "linux"))]
pub fn now() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_nanos() as u64
}

pub fn parse_stamp(text: &str) -> Option<u64> {
    let (secs, frac) = text.split_once('.').unwrap_or((text, ""));
    let secs: u64 = secs.parse().ok()?;
    if frac.len() > 9 || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let frac: u64 = format!("{frac:0<9}").parse().ok()?;
    Some(secs * 1_000_000_000 + frac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_read_to_the_nanosecond_and_windows_hold_their_ends() {
        assert_eq!(parse_stamp("112.158102"), Some(112_158_102_000));
        assert_eq!(parse_stamp("7.000000001"), Some(7_000_000_001));
        assert_eq!(parse_stamp("9"), Some(9_000_000_000));
        assert_eq!(parse_stamp("1.2x"), None);
        assert_eq!(parse_stamp(""), None);
        let w = [Window { start: 10, end: 20 }, Window { start: 30, end: 40 }];
        assert!(inside(&w, 10) && inside(&w, 20) && inside(&w, 35));
        assert!(!inside(&w, 25) && !inside(&w, 41) && !inside(&w, 9));
        assert!((seconds(&w) - 20e-9).abs() < 1e-15);
        let (a, b) = (now(), now());
        assert!(b >= a, "a steady clock");
    }
}
