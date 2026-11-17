//! macOS: `sample`, the system's own sampler (part of macOS, no install),
//! attached to this process for one viewpoint's measured frames and stopped
//! with SIGINT, which makes it write its report. The report's call graph is an
//! indented tree of `count symbol (in image) ...` lines, which [`parse`] folds:
//! a node's self samples are its count less its children's.

use std::path::{Path, PathBuf};
// lint: allow(subprocess) the bench drives the platform's profiler, `sample`, attached to itself
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::folded::Folded;

/// A `sample` running on this process.
pub struct Recording {
    child: Child,
    report: PathBuf,
}

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
    /// Stop sampling and fold the report it wrote.
    pub fn stop(mut self) -> Result<Folded, String> {
        // lint: allow(subprocess) SIGINT is how sample is told to stop and write its report
        let _ = Command::new("kill")
            .args(["-INT", &self.child.id().to_string()])
            .status();
        let _ = self.child.wait();
        let text = std::fs::read_to_string(&self.report)
            .map_err(|e| format!("sample wrote no report ({e})"))?;
        let _ = std::fs::remove_file(&self.report);
        Ok(parse(&text))
    }
}

/// The call graph of a `sample` report into folded stacks, each thread's root
/// line left out (its name is no function).
pub fn parse(text: &str) -> Folded {
    let mut folded = Folded::default();
    // (column, name, count, children's count) from the root down.
    let mut stack: Vec<(usize, String, u64, u64)> = Vec::new();
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
        while stack.last().is_some_and(|(c, ..)| *c >= col) {
            pop(&mut stack, &mut folded);
        }
        if let Some(parent) = stack.last_mut() {
            parent.3 += count;
        }
        let name = symbol.split("  (in ").next().unwrap_or(symbol).trim();
        stack.push((col, name.to_string(), count, 0));
    }
    while !stack.is_empty() {
        pop(&mut stack, &mut folded);
    }
    folded
}

/// Close the deepest open node: its own samples are a stack of their own.
fn pop(stack: &mut Vec<(usize, String, u64, u64)>, folded: &mut Folded) {
    let frames: Vec<String> = (stack.iter().skip(1)).map(|(_, n, ..)| n.clone()).collect();
    if let Some((_, _, count, children)) = stack.pop() {
        folded.add(&frames, count.saturating_sub(children));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_graph_folds_each_nodes_own_samples() {
        let text = "Analysis of sampling bench-scenes (pid 42) every 1 millisecond\n\
Call graph:\n\
    10 Thread_123   DispatchQueue_1: com.apple.main-thread  (serial)\n\
    + 10 start  (in dyld) + 1903  [0x18c]\n\
    +   10 main  (in bench-scenes) + 50  [0x100]\n\
    +     6 rusty_render::Renderer::render  (in bench-scenes) + 12  [0x200]\n\
    +     ! 4 rusty_render::draw::stack  (in bench-scenes) + 8  [0x300]\n\
    +     3 step  (in bench-scenes) + 4  [0x400]\n\
\n\
Total number in stack (recursive counted multiple times):\n";
        let f = parse(text);
        assert_eq!(f.samples(), 10, "{:?}", f.0);
        let render = "start;main;rusty_render::Renderer::render";
        assert_eq!(
            f.0.get(&format!("{render};rusty_render::draw::stack")),
            Some(&4)
        );
        assert_eq!(f.0.get(render), Some(&2));
        assert_eq!(f.0.get("start;main;step"), Some(&3));
        assert_eq!(f.0.get("start;main"), Some(&1));
    }
}
