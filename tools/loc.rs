//! loc.rs — throwaway curiosity tool: which .rs files / src subtrees are biggest.
//! NOT part of the build, NOT run in CI, NOT maintained. Run from the repo root:
//!   rustc -O tools/loc.rs -o /tmp/loc && /tmp/loc

use std::fs;
use std::path::{Path, PathBuf};

/// One `.rs` file and its line count.
struct FileLoc {
    path: PathBuf,
    lines: usize,
}

/// Line count of a single file (0 if unreadable / not UTF-8).
fn count_lines(path: &Path) -> usize {
    fs::read_to_string(path).map(|s| s.lines().count()).unwrap_or(0)
}

/// Recursively collect every `.rs` file under `root`, skipping `target` and hidden dirs.
fn collect(root: &Path, out: &mut Vec<FileLoc>) {
    let Ok(entries) = fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "target" {
            continue;
        }
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let lines = count_lines(&path);
            out.push(FileLoc { path, lines });
        }
    }
}

fn main() {
    let mut files = Vec::new();
    collect(Path::new("."), &mut files);

    // 1) Biggest files.
    files.sort_by(|a, b| b.lines.cmp(&a.lines));
    println!("== Top 20 biggest .rs files ==");
    for f in files.iter().take(20) {
        println!("{:>6}  {}", f.lines, f.path.display());
    }

    // 2) Totals.
    let total: usize = files.iter().map(|f| f.lines).sum();
    println!("\n== Totals ==");
    println!("{:>6}  .rs files", files.len());
    println!("{:>6}  total .rs lines", total);

    // 3) Immediate children of src/ (dirs aggregated recursively; direct .rs files as-is).
    println!("\n== src/ immediate children ==");
    let mut rows: Vec<(String, usize)> = Vec::new();
    if let Ok(entries) = fs::read_dir("src") {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let lines = if path.is_dir() {
                let mut sub = Vec::new();
                collect(&path, &mut sub);
                sub.iter().map(|f| f.lines).sum()
            } else if path.extension().is_some_and(|e| e == "rs") {
                count_lines(&path)
            } else {
                continue;
            };
            rows.push((name, lines));
        }
    }
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    for (name, lines) in rows {
        println!("{:>6}  {}", lines, name);
    }
}
