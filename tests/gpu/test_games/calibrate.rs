//! The yardstick the test games' budget is measured in: CPU time of [`TICKS`] ticks of the
//! default scene in a fresh [`Harness`], sampled in slices while the game runs ([`Sampler`]).
//! It meets the machine load the game does, so a game's CPU divided by it reads its own work.
//! A synthetic workload met load far harder than the game did.

use std::path::Path;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use rusty::dev::harness::Harness;

const BOT: &str = "assets/scripts/bot.lua";

const TICKS: u32 = 60;

pub(super) const IDLE: Duration = Duration::from_millis(1500);

/// Slices a run always gets, taken after the game if it ends sooner.
const MIN_SAMPLES: usize = 5;

pub(super) struct Sampler {
    stop: mpsc::Sender<()>,
    thread: JoinHandle<Vec<Duration>>,
}

impl Sampler {
    pub fn start() -> Self {
        let (stop, stopped) = mpsc::channel::<()>();
        // The test's temp dir is found by its thread's name, so on this thread.
        let out = crate::temp::dir().join("yardstick");
        let thread = std::thread::spawn(move || {
            let mut taken = Vec::new();
            loop {
                taken.push(slice(&out));
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

    pub fn finish(self) -> Vec<Duration> {
        let _ = self.stop.send(());
        self.thread.join().expect("the yardstick sampler")
    }
}

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

pub(super) fn samples(count: usize) -> Vec<Duration> {
    let out = crate::temp::dir().join("yardstick");
    (0..count).map(|_| slice(&out)).collect()
}

pub(super) fn median(samples: &[Duration]) -> Duration {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

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
    // SAFETY: `time` is a live `struct timespec` (64-bit Linux and macOS).
    let read = unsafe { clock_gettime(CLOCK_THREAD_CPUTIME_ID, &raw mut time) };
    assert_eq!(
        read,
        0,
        "clock_gettime: {}",
        std::io::Error::last_os_error()
    );
    Duration::new(time.sec as u64, time.nsec as u32)
}
