//! The yardstick the test games' budget is measured in: the CPU time this
//! thread spends on one fixed, deterministic workload. Sampled right before and
//! right after a game, under the same load, it rises and falls with what the
//! load does to CPU time (a shared SMT core, all-core clocks), so a game's CPU
//! time divided by it reads the game's own work.
//!
//! The workload is single-threaded integer and float work over a buffer that
//! fits a core's L2: the shared core and the clock reach it as they reach a
//! game, while a buffer past L2 would read the neighbours' memory traffic, which
//! varied it by half again between idle runs. One sample is short and noisy, so
//! the yardstick is the median of many.

use std::time::Duration;

/// Words in the buffer: 1 MiB, inside a desktop or server core's L2.
const WORDS: usize = 1 << 18;

/// Scattered steps over the buffer per sample: about a tenth of a second of
/// CPU in the test profile on a container core.
const STEPS: u32 = 1 << 23;

/// Thread CPU time of `count` runs of the workload, one each.
pub(super) fn samples(count: usize) -> Vec<Duration> {
    let mut words: Vec<u32> = (0..WORDS as u32).collect();
    (0..count)
        .map(|_| {
            let start = thread_cpu();
            let checksum = workload(&mut words);
            let spent = thread_cpu().saturating_sub(start);
            // Kept, so the optimiser cannot drop the work.
            assert!(
                checksum.is_finite(),
                "the yardstick's workload stays finite"
            );
            spent
        })
        .collect()
}

/// The median of `samples`, which a stray stall in one or two does not move.
pub(super) fn median(samples: &[Duration]) -> Duration {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

/// The fixed work: an LCG walks the buffer, each read word mixing into a float
/// accumulator and the next write. Same steps, same addresses on every run.
fn workload(words: &mut [u32]) -> f64 {
    let mut state: u32 = 0x2243;
    let mut acc = 0.0f64;
    for _ in 0..STEPS {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let at = (state >> 10) as usize & (WORDS - 1);
        let word = words[at];
        acc = acc * 0.999_9 + f64::from(word & 0xffff) * 1e-6;
        words[(at ^ 0x5a5a) & (WORDS - 1)] = word.rotate_left(7) ^ state;
    }
    std::hint::black_box(&words);
    acc
}

/// This thread's CPU time, from `clock_gettime(CLOCK_THREAD_CPUTIME_ID)`, which
/// Linux and macOS both have, declared here as `measure` declares `wait4`.
fn thread_cpu() -> Duration {
    #[repr(C)]
    struct Timespec {
        sec: i64,
        nsec: i64,
    }
    #[cfg(target_os = "macos")]
    const CLOCK_THREAD_CPUTIME_ID: i32 = 16;
    #[cfg(not(target_os = "macos"))]
    const CLOCK_THREAD_CPUTIME_ID: i32 = 3;
    unsafe extern "C" {
        fn clock_gettime(clock: i32, time: *mut Timespec) -> i32;
    }
    let mut time = Timespec { sec: 0, nsec: 0 };
    // SAFETY: `time` is live, writable and laid out as `struct timespec` on
    // 64-bit Linux and macOS.
    let read = unsafe { clock_gettime(CLOCK_THREAD_CPUTIME_ID, &raw mut time) };
    assert_eq!(
        read,
        0,
        "clock_gettime: {}",
        std::io::Error::last_os_error()
    );
    Duration::new(time.sec as u64, time.nsec as u32)
}
