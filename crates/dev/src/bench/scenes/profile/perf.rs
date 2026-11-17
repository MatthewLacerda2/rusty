//! Linux: `perf record` attached to this process for one viewpoint's measured
//! frames, user space only (`cycles:u`, else the `cpu-clock:u` timer where the
//! machine has no cycle counter, a VM), so `kernel.perf_event_paranoid` up to 2
//! lets it run unprivileged. The call stacks are unwound from DWARF (a release
//! build keeps no frame pointers), and `perf script` prints them for [`parse`].

use std::path::{Path, PathBuf};
// lint: allow(subprocess) the bench drives the platform's profiler, `perf`, attached to itself
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::folded::Folded;

/// Samples a second: enough for a few thousand samples a viewpoint, few enough
/// that the DWARF stacks stay small.
const HZ: &str = "499";
/// The events tried in order: the cycle counter, then the kernel's timer.
const EVENTS: [&str; 2] = ["cycles:u", "cpu-clock:u"];

/// A `perf record` running on this process.
pub struct Recording {
    perf: PathBuf,
    child: Child,
    data: PathBuf,
    pub event: &'static str,
}

/// Start recording this process into `data`, with the `perf` at `perf`.
pub fn start(perf: &Path, data: &Path) -> Result<Recording, String> {
    let mut refused = String::new();
    for event in EVENTS {
        let pid = std::process::id().to_string();
        let child = Command::new(perf)
            .args([
                "record",
                "-q",
                "-e",
                event,
                "-F",
                HZ,
                "--call-graph",
                "dwarf,16384",
            ])
            .args(["-p", &pid, "-o"])
            .arg(data)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
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
                    event,
                });
            }
            _ => refused = stderr_of(child),
        }
    }
    Err(refusal(&refused))
}

impl Recording {
    /// Stop recording (perf writes its file on SIGINT) and fold what it sampled.
    pub fn stop(mut self) -> Result<Folded, String> {
        // lint: allow(subprocess) SIGINT is how perf record is told to finish its file
        let sent = Command::new("kill")
            .args(["-INT", &self.child.id().to_string()])
            .status();
        if !sent.is_ok_and(|s| s.success()) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        let script = Command::new(&self.perf)
            .args(["script", "-F", "ip,sym", "-i"])
            .arg(&self.data)
            .stderr(Stdio::null())
            .output()
            .map_err(|e| format!("perf script: {e}"))?;
        let _ = std::fs::remove_file(&self.data);
        if !script.status.success() {
            return Err("perf script could not read the recording".into());
        }
        Ok(parse(&String::from_utf8_lossy(&script.stdout)))
    }
}

fn stderr_of(child: Child) -> String {
    let out = child.wait_with_output();
    out.map_or(String::new(), |o| {
        String::from_utf8_lossy(&o.stderr).into_owned()
    })
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

/// `perf script -F ip,sym` text into folded stacks: one sample per block of
/// frame lines (`<hex ip> <symbol>`, leaf first), blocks parted by a blank line.
pub fn parse(text: &str) -> Folded {
    let mut folded = Folded::default();
    let mut frames: Vec<String> = Vec::new();
    let mut flush = |frames: &mut Vec<String>| {
        frames.reverse();
        folded.add(frames, 1);
        frames.clear();
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            flush(&mut frames);
            continue;
        }
        let Some((ip, sym)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if u64::from_str_radix(ip, 16).is_ok() {
            frames.push(sym.trim().to_string());
        }
    }
    flush(&mut frames);
    folded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_blocks_fold_root_first_with_unknown_frames_kept() {
        let text = "\t    55d0c0ffee10 rusty_render::Renderer::render\n\
                    \t    55d0c0ffee00 rusty_dev::bench::measure_posed\n\
                    \t    55d0c0ffe000 main\n\
                    \n\
                    \t    7f0000000010 [unknown]\n\
                    \t    55d0c0ffe000 main\n\
                    \n\
                    \t    55d0c0ffee10 rusty_render::Renderer::render\n\
                    \t    55d0c0ffee00 rusty_dev::bench::measure_posed\n\
                    \t    55d0c0ffe000 main\n";
        let f = parse(text);
        assert_eq!(f.samples(), 3);
        let stack = "main;rusty_dev::bench::measure_posed;rusty_render::Renderer::render";
        assert_eq!(f.0.get(stack), Some(&2));
        assert_eq!(f.0.get("main;[unknown]"), Some(&1));
    }

    #[test]
    fn a_paranoid_refusal_says_what_to_set() {
        let said =
            refusal("Error: Access to performance monitoring is limited by perf_event_paranoid");
        assert!(
            said.contains("sudo sysctl kernel.perf_event_paranoid=2"),
            "{said}"
        );
    }
}
