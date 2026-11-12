//! Run a child and read what it cost: its own CPU time (user + system), which
//! the test games' budget reads because other tests' load barely moves it, and
//! its wall time, kept only as a hang guard (#2131). The test reaps the child
//! itself with `wait4`, which hands back the child's rusage on Linux and macOS
//! alike, so neither the `game` binary nor a new crate has to report it.

use std::io::Read;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};

/// What a finished child printed and cost; `killed` when it ran past the ceiling.
pub(super) struct Measured {
    pub output: Output,
    pub cpu: Duration,
    pub wall: Duration,
    pub killed: bool,
}

/// Run `command` to its end, or kill it once its wall time passes `ceiling`.
pub(super) fn run(command: &mut Command, ceiling: Duration) -> Measured {
    let start = Instant::now();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the child starts");
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let (status, cpu, killed) = reap(&mut child, start, ceiling);
    let wall = start.elapsed();
    let output = Output {
        status,
        stdout: stdout.join().expect("stdout reader"),
        stderr: stderr.join().expect("stderr reader"),
    };
    Measured {
        output,
        cpu,
        wall,
        killed,
    }
}

/// Read a pipe to its end on a thread of its own, so a chatty child never blocks.
fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            pipe.read_to_end(&mut bytes)
                .expect("the child's pipe reads");
        }
        bytes
    })
}

#[cfg(unix)]
fn reap(child: &mut Child, start: Instant, ceiling: Duration) -> (ExitStatus, Duration, bool) {
    use std::os::unix::process::ExitStatusExt;
    let pid = i32::try_from(child.id()).expect("a pid fits pid_t");
    let mut killed = false;
    loop {
        let blocking = killed;
        if let Some((status, cpu)) = rusage::reap(pid, blocking) {
            return (ExitStatus::from_raw(status), cpu, killed);
        }
        if start.elapsed() > ceiling {
            child.kill().expect("the hung child is killed");
            killed = true;
        } else {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Off Unix there is no rusage: the child's wall time stands in for its CPU time.
#[cfg(not(unix))]
fn reap(child: &mut Child, start: Instant, ceiling: Duration) -> (ExitStatus, Duration, bool) {
    let mut killed = false;
    loop {
        if let Some(status) = child.try_wait().expect("the child waits") {
            return (status, start.elapsed(), killed);
        }
        if start.elapsed() > ceiling && !killed {
            child.kill().expect("the hung child is killed");
            killed = true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// `wait4(2)`, declared here: libc links into every Rust program on Unix, and
/// `struct rusage` has the same 144-byte layout on 64-bit Linux and macOS but
/// for `tv_usec`'s width.
#[cfg(unix)]
mod rusage {
    use std::time::Duration;

    #[repr(C)]
    struct Timeval {
        sec: i64,
        #[cfg(target_os = "macos")]
        usec: i32,
        #[cfg(not(target_os = "macos"))]
        usec: i64,
    }

    #[repr(C)]
    struct Rusage {
        user: Timeval,
        system: Timeval,
        rest: [i64; 14],
    }

    const _: () = assert!(size_of::<Rusage>() == 144, "struct rusage is 144 bytes");
    const WNOHANG: i32 = 1;

    unsafe extern "C" {
        fn wait4(pid: i32, status: *mut i32, options: i32, rusage: *mut Rusage) -> i32;
    }

    /// Reap `pid` (waiting for it when `blocking`), with its CPU time (user +
    /// system), or `None` while it still runs.
    pub(super) fn reap(pid: i32, blocking: bool) -> Option<(i32, Duration)> {
        let mut status = 0;
        let mut usage = std::mem::MaybeUninit::<Rusage>::zeroed();
        let options = if blocking { 0 } else { WNOHANG };
        // SAFETY: `status` and `usage` are live, writable and sized as wait4 expects.
        let reaped = unsafe { wait4(pid, &raw mut status, options, usage.as_mut_ptr()) };
        assert!(reaped >= 0, "wait4: {}", std::io::Error::last_os_error());
        if reaped == 0 {
            return None;
        }
        // SAFETY: zeroed, then filled by a successful wait4.
        let usage = unsafe { usage.assume_init() };
        let time = |t: &Timeval| Duration::new(t.sec as u64, t.usec as u32 * 1000);
        Some((status, time(&usage.user) + time(&usage.system)))
    }
}
