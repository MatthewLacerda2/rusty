//! The yardstick the test games' budget is measured in: the CPU time a test
//! thread spends stepping a fixed headless scene, sampled in slices for as long
//! as the game runs ([`Sampler`]). Inside the game's own window it meets the same
//! load the game does (a shared SMT core, all-core clocks, a contended cache), so
//! a game's CPU time divided by it reads the game's own work.
//!
//! **Why the engine's own work.** A game's CPU is mostly its sim's fixed tick on
//! the main thread: `runner`'s profile on lavapipe (4-vCPU container, the test
//! profile) put about three quarters of it in the tick (physics most of all,
//! then perception, the UI's events and the Lua scripts), a tenth in drawing its
//! frames, and under 4% on lavapipe's own threads. So a slice is that tick and
//! nothing else: [`TICKS`] ticks of the engine's default scene (its player, its
//! bot with perception and a Lua brain, crates and water under physics) in a
//! fresh [`Harness`], built untimed before each slice so every slice does the
//! same work. A synthetic workload (an LCG walk over a 1 MiB buffer) met load
//! far harder than the game did: on an 8-core SMT machine a cold `cargo build`
//! swelled it 1.7 to 5.9 times while the game's CPU rose 1.3 to 1.9 times.
//!
//! The yardstick is the [`median`] slice: over eight `runner` runs in the
//! container (alone, beside a cold `cargo build`, beside spinners and memory
//! thrashers) its count spread 1.035×, where the lower quartile's spread 1.07×
//! and a 20% trimmed mean's 1.06×.
//!
//! **The sampler's own cost**: a slice of about 0.16 s of CPU plus an untimed
//! harness build of about 0.08 s, per [`IDLE`] of sleep: about 14% of one core
//! (4-vCPU container, the test profile), competing with the game it measures.

use std::path::Path;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use rusty::dev::harness::Harness;

/// The enemy brain the default scene's bot runs, as the harness tests name it.
const BOT: &str = "assets/scripts/bot.lua";

/// Fixed ticks per slice: 0.13 to 0.24 s of CPU, median about 0.165 s, in the
/// test profile on a 4-vCPU container core.
const TICKS: u32 = 60;

/// Sleep between two slices.
pub(super) const IDLE: Duration = Duration::from_millis(1500);

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
        // The test's temp dir is found by its thread's name, so on this thread.
        let out = crate::temp::dir().join("yardstick");
        let thread = std::thread::spawn(move || {
            let mut taken = Vec::new();
            loop {
                taken.push(slice(&out));
                // A message or a dropped sender both end the window.
                if stopped.recv_timeout(IDLE) != Err(mpsc::RecvTimeoutError::Timeout) {
                    break;
                }
            }
            while taken.len() < MIN_SAMPLES {
                taken.push(slice(&out));
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

/// Thread CPU time of [`TICKS`] ticks of a fresh default-scene harness in `out`;
/// building the harness is not timed.
fn slice(out: &Path) -> Duration {
    let harness = Harness::new(out, BOT);
    let start = thread_cpu();
    harness.step(TICKS);
    let spent = thread_cpu().saturating_sub(start);
    assert_eq!(
        harness.frame(),
        u64::from(TICKS),
        "the yardstick's scene steps every tick"
    );
    spent
}

/// Thread CPU time of `count` slices, back to back, on this thread.
pub(super) fn samples(count: usize) -> Vec<Duration> {
    let out = crate::temp::dir().join("yardstick");
    (0..count).map(|_| slice(&out)).collect()
}

/// The median of `samples`, which a stray stall in one or two does not move.
pub(super) fn median(samples: &[Duration]) -> Duration {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
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
