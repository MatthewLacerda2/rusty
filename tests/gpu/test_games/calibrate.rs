//! The yardstick the test games' budget is measured in: the CPU time a test
//! thread spends on one fixed, deterministic workload, sampled in short slices
//! for as long as the game runs ([`Sampler`]). Inside the game's own window it
//! meets the same load the game does (a shared SMT core, all-core clocks), so a
//! game's CPU time divided by it reads the game's own work. Slices taken only
//! before and after the game caught moments its run never saw, and spread more
//! than the game's raw CPU time did (#2243).
//!
//! The workload is single-threaded integer and float work over a buffer that
//! fits a core's L2: the shared core and the clock reach it as they reach a
//! game, while a buffer past L2 would read the neighbours' memory traffic, which
//! varied it by half again between idle runs. One slice is short and noisy, so
//! the yardstick is the median of many.
//!
//! **The sampler's own cost**: one slice (about 0.07 s of CPU in the test
//! profile on a 4-vCPU container core) per [`IDLE`] of sleep, so about 7% of
//! one core, competing with the game it measures.

use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

/// Words in the buffer: 1 MiB, inside a desktop or server core's L2.
const WORDS: usize = 1 << 18;

/// Scattered steps over the buffer per slice: about 0.07 s of CPU in the test
/// profile on a 4-vCPU container core, 0.065 to 0.11 s on CI's runners.
const STEPS: u32 = 1 << 23;

/// Sleep between two slices: with a slice of about 0.07 s, a duty of about 7%.
pub(super) const IDLE: Duration = Duration::from_millis(900);

/// Slices a run always gets, taken after the game when it ends sooner (the
/// `smoke` game takes two seconds), so the median is never of one or two.
const MIN_SAMPLES: usize = 5;

/// A thread taking a yardstick slice every [`IDLE`] until [`Sampler::finish`].
pub(super) struct Sampler {
    stop: mpsc::Sender<()>,
    thread: JoinHandle<Vec<Duration>>,
}

impl Sampler {
    /// Start sampling, the first slice at once.
    pub fn start() -> Self {
        let (stop, stopped) = mpsc::channel::<()>();
        let thread = std::thread::spawn(move || {
            let mut words = buffer();
            let mut taken = Vec::new();
            loop {
                taken.push(slice(&mut words));
                // A message or a dropped sender both end the window.
                if stopped.recv_timeout(IDLE) != Err(mpsc::RecvTimeoutError::Timeout) {
                    break;
                }
            }
            while taken.len() < MIN_SAMPLES {
                taken.push(slice(&mut words));
            }
            taken
        });
        Self { stop, thread }
    }

    /// Stop sampling and hand back every slice taken, at least [`MIN_SAMPLES`].
    pub fn finish(self) -> Vec<Duration> {
        let _ = self.stop.send(());
        self.thread.join().expect("the yardstick sampler")
    }
}

fn buffer() -> Vec<u32> {
    (0..WORDS as u32).collect()
}

/// Thread CPU time of one run of the workload.
fn slice(words: &mut [u32]) -> Duration {
    let start = thread_cpu();
    let checksum = workload(words);
    let spent = thread_cpu().saturating_sub(start);
    // Kept, so the optimiser cannot drop the work.
    assert!(
        checksum.is_finite(),
        "the yardstick's workload stays finite"
    );
    spent
}

/// Thread CPU time of `count` runs of the workload, one each, back to back.
pub(super) fn samples(count: usize) -> Vec<Duration> {
    let mut words = buffer();
    (0..count).map(|_| slice(&mut words)).collect()
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
