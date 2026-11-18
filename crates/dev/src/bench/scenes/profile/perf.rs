//! Linux: one `perf record` attached to this process for a whole scene, user
//! space only (`cycles:u`, else the `cpu-clock:u` timer where the machine has no
//! cycle counter, a VM), so `kernel.perf_event_paranoid` up to 2 lets it run
//! unprivileged. Its samples are stamped with `CLOCK_MONOTONIC`, and [`parse`]
//! keeps those inside the viewpoints' measured windows ([`Window`]), so the
//! warm-ups between them never count. The call stacks are walked by frame
//! pointer (the `profiling` build forces them on), a few words a sample where
//! the 16 KiB DWARF stack copies used before kept perf's ring buffer far busier
//! (the recordings that held them lost most of their samples; which of their
//! settings lost them was not isolated). A walk through code built without
//! frame pointers (the system libraries, a GPU driver) can end a stack early,
//! or invent a caller where that code keeps something else in the frame
//! register; the leaf, and so the self time, is always right. An unnamed frame
//! is named by its image ([`trust::frame_name`]).

use std::path::{Path, PathBuf};
// lint: allow(subprocess) the bench drives the platform's profiler, `perf`, attached to itself
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::folded::Folded;
use super::trust::{self, Tally};
use super::window::{self, Window};

/// Samples a second: a few thousand a viewpoint.
pub const HZ: u32 = 499;
/// The events tried in order: the cycle counter, then the kernel's timer.
const EVENTS: [&str; 2] = ["cycles:u", "cpu-clock:u"];

/// A `perf record` running on this process.
pub struct Recording {
    perf: PathBuf,
    child: Child,
    data: PathBuf,
    /// What perf prints while it records (lost samples, its summary).
    log: PathBuf,
    pub event: &'static str,
}

/// Start recording this process into `data`, with the `perf` at `perf`.
pub fn start(perf: &Path, data: &Path) -> Result<Recording, String> {
    let log = data.with_extension("log");
    let mut refused = String::new();
    for event in EVENTS {
        let pid = std::process::id().to_string();
        // A file, not a pipe: a pipe nobody reads until the end could fill and stall perf.
        // lint: allow(asset-write) perf's own log, read back and deleted by this module
        let stderr = std::fs::File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
        let child = Command::new(perf)
            .args(["record", "-e", event, "-F", &HZ.to_string()])
            .args(["--call-graph", "fp", "-k", "CLOCK_MONOTONIC"])
            .args(["-p", &pid, "-o"])
            .arg(data)
            .stdout(Stdio::null())
            .stderr(stderr)
            .spawn()
            .map_err(|e| format!("cannot run {}: {e}", perf.display()))?;
        let mut child = child;
        // perf attaches within a moment, or exits saying why it cannot.
        std::thread::sleep(Duration::from_millis(500));
        match child.try_wait() {
            Ok(None) => {
                let (perf, data) = (perf.to_path_buf(), data.to_path_buf());
                return Ok(Recording {
                    perf,
                    child,
                    data,
                    log,
                    event,
                });
            }
            _ => refused = std::fs::read_to_string(&log).unwrap_or_default(),
        }
    }
    Err(refusal(&refused))
}

impl Recording {
    /// Stop recording (perf writes its file on SIGINT) and fold what it sampled
    /// inside `windows`; a leaf in the image named `own` is the bench's own.
    pub fn stop(mut self, windows: &[Window], own: &str) -> Result<(Folded, Tally), String> {
        // lint: allow(subprocess) SIGINT is how perf record is told to finish its file
        let sent = Command::new("kill")
            .args(["-INT", &self.child.id().to_string()])
            .status();
        if !sent.is_ok_and(|s| s.success()) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        let script = Command::new(&self.perf)
            .args(["script", "-F", "time,ip,sym,dso", "--no-inline", "-i"])
            .arg(&self.data)
            .output()
            .map_err(|e| format!("perf script: {e}"))?;
        let recorded = std::fs::read_to_string(&self.log).unwrap_or_default();
        let _ = std::fs::remove_file(&self.data);
        let _ = std::fs::remove_file(&self.log);
        if !script.status.success() {
            let said = String::from_utf8_lossy(&script.stderr);
            return Err(format!(
                "perf script could not read the recording: {}",
                said.trim()
            ));
        }
        let (folded, mut tally) = parse(&String::from_utf8_lossy(&script.stdout), windows, own);
        let said = format!("{recorded}\n{}", String::from_utf8_lossy(&script.stderr));
        tally.warnings.extend(warnings(&said));
        Ok((folded, tally))
    }
}

/// The lines of what perf printed that say samples went missing.
fn warnings(said: &str) -> Vec<String> {
    let lost = |l: &&str| l.contains("lost") || l.contains("LOST");
    let mut out: Vec<String> = said
        .lines()
        .filter(lost)
        .map(|l| format!("perf: {}", l.trim()))
        .collect();
    out.dedup();
    out
}

/// What to say when perf would not record, from what it printed.
fn refusal(stderr: &str) -> String {
    let paranoid = std::fs::read_to_string("/proc/sys/kernel/perf_event_paranoid");
    let level = paranoid.as_deref().map_or("unknown", str::trim);
    let mut out = format!("perf would not record this process: {}", stderr.trim());
    if stderr.contains("paranoid") || stderr.contains("ermission") {
        out.push_str(&format!(
            "\nkernel.perf_event_paranoid is {level}; user-space sampling of your own \
             process needs 2 or lower: sudo sysctl kernel.perf_event_paranoid=2"
        ));
    }
    if stderr.contains("not found for kernel") {
        out.push_str(
            "\nthis perf is a wrapper without the tools for the running kernel: \
                      install linux-tools-$(uname -r) (Ubuntu) or linux-perf (Debian)",
        );
    }
    out
}

/// `perf script -F time,ip,sym,dso` text into folded stacks, keeping the samples
/// stamped inside one of `windows`. A sample is a header line (`<seconds>:`,
/// with its one frame on it when there is no call stack) then its frame lines
/// (`<hex ip> <symbol> (<image>)`, leaf first), parted by a blank line.
pub fn parse(text: &str, windows: &[Window], own: &str) -> (Folded, Tally) {
    let mut folded = Folded::default();
    let mut tally = Tally::default();
    // The sample being read: whether it is kept, and its frames as (symbol, image).
    let mut keep = false;
    let mut frames: Vec<(String, String)> = Vec::new();
    let mut flush = |keep: bool, frames: &mut Vec<(String, String)>| {
        if keep && !frames.is_empty() {
            let (sym, image) = &frames[0];
            tally.count(image, sym, own, 1);
            let names: Vec<String> = (frames.iter().rev())
                .map(|(s, image)| trust::frame_name(s, image))
                .collect();
            folded.add(&names, 1);
        }
        frames.clear();
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            flush(keep, &mut frames);
            keep = false;
            continue;
        }
        let frame = match stamp(line) {
            Some((t, rest)) => {
                flush(keep, &mut frames);
                keep = window::inside(windows, t);
                rest
            }
            None => line,
        };
        frames.extend(parse_frame(frame));
    }
    flush(keep, &mut frames);
    (folded, tally)
}

/// A sample's header: its stamp in nanoseconds, and what follows on the line.
fn stamp(line: &str) -> Option<(u64, &str)> {
    let (head, rest) = line.split_once(':')?;
    // Only the stamp stands before the colon: a frame's `::` follows its ip and a space.
    let t = window::parse_stamp(head.trim())?;
    Some((t, rest.trim()))
}

/// One frame line, `<hex ip> <symbol> (<image>)`, into (symbol, image).
fn parse_frame(line: &str) -> Option<(String, String)> {
    let (ip, rest) = line.split_once(char::is_whitespace)?;
    u64::from_str_radix(ip, 16).ok()?;
    let rest = rest.trim();
    let (sym, image) = match rest.rfind(" (") {
        Some(at) if rest.ends_with(')') => (&rest[..at], &rest[at + 2..rest.len() - 1]),
        _ => (rest, ""),
    };
    Some((sym.trim().to_string(), image.to_string()))
}

#[cfg(test)]
#[path = "perf_tests.rs"]
mod tests;
