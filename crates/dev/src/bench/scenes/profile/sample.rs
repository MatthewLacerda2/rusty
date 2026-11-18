//! macOS: `sample`, the system's own sampler (part of macOS, no install),
//! attached to this process for one viewpoint's measured frames and stopped
//! with SIGINT, which makes it write its report. The report's call graph is an
//! indented tree of `count symbol (in image) ...` lines, which [`parse`] folds:
//! a node's self samples are its count less its children's. A frame `sample`
//! cannot name (`???`) is one with no symbol, named in the tables by its image.

use std::path::{Path, PathBuf};
// lint: allow(subprocess) the bench drives the platform's profiler, `sample`, attached to itself
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::folded::Folded;
use super::trust::{frame_name, Tally, UNKNOWN};

/// A `sample` running on this process.
pub struct Recording {
    child: Child,
    report: PathBuf,
}

/// Samples a second: `sample` samples every thread each millisecond.
pub const HZ: u32 = 1000;

/// Start sampling this process (every millisecond, for up to an hour) into `report`.
pub fn start(sample: &Path, report: &Path) -> Result<Recording, String> {
    let pid = std::process::id().to_string();
    let mut child = Command::new(sample)
        .args([pid.as_str(), "3600", "1", "-mayDie", "-file"])
        .arg(report)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run {}: {e}", sample.display()))?;
    std::thread::sleep(Duration::from_millis(500));
    if let Ok(Some(_)) = child.try_wait() {
        let out = child.wait_with_output();
        let said = out.map_or(String::new(), |o| {
            String::from_utf8_lossy(&o.stderr).into_owned()
        });
        return Err(format!(
            "sample would not attach to this process: {}",
            said.trim()
        ));
    }
    let report = report.to_path_buf();
    Ok(Recording { child, report })
}

impl Recording {
    /// Stop sampling and fold the report it wrote; a leaf in the image named
    /// `own` is the bench's own.
    pub fn stop(mut self, own: &str) -> Result<(Folded, Tally), String> {
        // lint: allow(subprocess) SIGINT is how sample is told to stop and write its report
        let _ = Command::new("kill")
            .args(["-INT", &self.child.id().to_string()])
            .status();
        let _ = self.child.wait();
        let text = std::fs::read_to_string(&self.report)
            .map_err(|e| format!("sample wrote no report ({e})"))?;
        let _ = std::fs::remove_file(&self.report);
        Ok(parse(&text, own))
    }
}

/// The thread the bench runs on: its harness, its sim and its renderer's CPU
/// side all run on the main thread.
const BENCH_THREAD: &str = "com.apple.main-thread";

/// The call graph of a `sample` report into folded stacks, the bench's own
/// thread (`BENCH_THREAD`; the first listed when none is named so) alone, and
/// its root line left out (its name is no function). `sample` samples every
/// thread every interval, running or parked, so the others (wgpu's, the audio
/// device's, the runtime's) would fill the table with their waits
/// (`__psynch_cvwait`, `mach_msg2_trap`); the bench thread's own waits (on the
/// GPU) are its time and stay. Each leaf is tallied by its image, the bench's
/// own named `own`.
pub fn parse(text: &str, own: &str) -> (Folded, Tally) {
    let mut threads: Vec<Thread> = Vec::new();
    let mut stack: Vec<Node> = Vec::new();
    let graph = text
        .lines()
        .skip_while(|l| !l.starts_with("Call graph:"))
        .skip(1);
    for line in graph.take_while(|l| !l.trim().is_empty()) {
        let Some(col) = line.find(|c: char| c.is_ascii_digit()) else {
            continue;
        };
        let rest = &line[col..];
        let Some((count, symbol)) = rest.split_once(' ') else {
            continue;
        };
        let Ok(count) = count.parse::<u64>() else {
            continue;
        };
        while stack.last().is_some_and(|n| n.col >= col) {
            pop(&mut stack, &mut threads, own);
        }
        match stack.last_mut() {
            Some(parent) => parent.children += count,
            None => threads.push((symbol.to_string(), Folded::default(), Tally::default())),
        }
        let (name, image) = symbol.split_once("  (in ").unwrap_or((symbol, ""));
        let image = image.split(')').next().unwrap_or_default();
        // `sample` prints `???` for a frame it cannot name: no symbol, as perf's `[unknown]`.
        let name = match name.trim() {
            "???" => UNKNOWN,
            named => named,
        };
        stack.push(Node {
            col,
            label: frame_name(name, image),
            name: name.to_string(),
            image: image.to_string(),
            count,
            children: 0,
        });
    }
    while !stack.is_empty() {
        pop(&mut stack, &mut threads, own);
    }
    let bench = threads
        .iter()
        .position(|(root, ..)| root.contains(BENCH_THREAD));
    let mut threads = threads.into_iter();
    let kept = match bench {
        Some(at) => threads.nth(at),
        None => threads.next(),
    };
    kept.map(|(_, folded, tally)| (folded, tally))
        .unwrap_or_default()
}

/// A thread of the report: its root line, its stacks and their tally.
type Thread = (String, Folded, Tally);

/// An open node of the call graph, from the root down.
struct Node {
    col: usize,
    /// The symbol, [`UNKNOWN`] when `sample` could not name it.
    name: String,
    /// What the tables call it ([`frame_name`]).
    label: String,
    image: String,
    count: u64,
    children: u64,
}

/// Close the deepest open node: its own samples are a stack of their own, in
/// the thread it is under (the last one opened), with it as the leaf.
fn pop(stack: &mut Vec<Node>, threads: &mut [Thread], own: &str) {
    let frames: Vec<String> = (stack.iter().skip(1)).map(|n| n.label.clone()).collect();
    let (Some(node), Some((_, folded, tally))) = (stack.pop(), threads.last_mut()) else {
        return;
    };
    let mine = node.count.saturating_sub(node.children);
    if mine > 0 && !frames.is_empty() {
        tally.count(&node.image, &node.name, own, mine);
    }
    folded.add(&frames, mine);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_graph_folds_the_bench_threads_own_samples_and_drops_parked_threads() {
        let text = "Analysis of sampling bench-scenes (pid 42) every 1 millisecond\n\
Call graph:\n\
    10 Thread_123   DispatchQueue_1: com.apple.main-thread  (serial)\n\
    + 10 start  (in dyld) + 1903  [0x18c]\n\
    +   10 main  (in bench-scenes) + 50  [0x100]\n\
    +     6 rusty_render::Renderer::render  (in bench-scenes) + 12  [0x200]\n\
    +     ! 4 rusty_render::draw::stack  (in bench-scenes) + 8  [0x300]\n\
    +     3 step  (in bench-scenes) + 4  [0x400]\n\
    10 Thread_124: wgpu  (serial)\n\
    + 10 thread_start  (in libsystem_pthread.dylib) + 8  [0x500]\n\
    +   10 __psynch_cvwait  (in libsystem_kernel.dylib) + 8  [0x600]\n\
\n\
Total number in stack (recursive counted multiple times):\n";
        let (f, tally) = parse(text, "bench-scenes");
        assert_eq!(f.samples(), 10, "{:?}", f.0);
        let render = "start;main;rusty_render::Renderer::render";
        assert_eq!(
            f.0.get(&format!("{render};rusty_render::draw::stack")),
            Some(&4)
        );
        assert_eq!(f.0.get(render), Some(&2));
        assert_eq!(f.0.get("start;main;step"), Some(&3));
        assert_eq!(f.0.get("start;main"), Some(&1));
        // Every leaf with samples of its own is in the bench's image (`start`, in dyld, has none).
        assert_eq!((tally.samples, tally.own), (10, 10), "{tally:?}");
        assert!(
            !f.0.keys().any(|k| k.contains("__psynch_cvwait")),
            "a parked thread's wait: {:?}",
            f.0
        );
    }

    #[test]
    fn an_unnamed_frame_has_no_symbol_and_is_named_by_its_image() {
        let text = "Call graph:\n\
    10 Thread_123   DispatchQueue_1: com.apple.main-thread  (serial)\n\
    + 10 main  (in bench-scenes) + 50  [0x100]\n\
    +   8 ???  (in AGXMetalG15X)  load address 0x1 + 0x2  [0x3]\n\
    +   2 ???  (in bench-scenes)  load address 0x4 + 0x5  [0x6]\n\
\n";
        let (f, tally) = parse(text, "bench-scenes");
        assert_eq!(
            (tally.samples, tally.own, tally.unknown),
            (10, 0, 10),
            "{tally:?}"
        );
        assert_eq!(f.0.get("main;[AGXMetalG15X]"), Some(&8), "{:?}", f.0);
        assert_eq!(f.0.get("main;[bench-scenes]"), Some(&2), "{:?}", f.0);
        assert!(super::super::trust::refusal(&tally, 0.0).is_some());
    }
}
