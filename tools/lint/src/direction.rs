//! Dependency-direction guard — the sim never imports the renderer or the editor.
//!
//! The simulation runs headless with no GPU and no UI: the arrow is render → sim and
//! editor → sim, never the other way (#494). Once a sim module names a `render` or
//! `editor` type — or a GPU/UI crate directly — the headless path compiles the
//! display layer again and the cycle a crate split (#495) would have to cut is back.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --direction`
//! Exit code 1 on any violation; report mirrored to `.lint/report.txt`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

/// Directories whose `.rs` files are sim code and must not depend on the display
/// layer. Wider than the determinism guard's list: `scene`, `components`, `ecs`,
/// `core`, `time` and `asset` hold the data the sim runs on; `ui` is the in-game
/// UI's layout, which runs headless in the sim (#417).
const SIM_DIRS: &[&str] = &[
    "src/app",
    "src/scripting",
    "src/physics",
    "src/navigation",
    "src/scene",
    "src/components",
    "src/ecs",
    "src/core",
    "src/time",
    "src/asset",
    "src/ui",
];

/// Banned paths, matched as whole path segments on non-comment source: the engine's
/// own display modules, and the GPU / UI crates by name.
const BANNED: &[&str] = &["crate::render", "crate::editor", "wgpu", "egui"];

const REPORT: &str = ".lint/report.txt";

/// Entry point: scan the sim modules and fail on any sim → display-layer reference.
pub fn run() {
    let mut violations = Vec::new();
    for dir in SIM_DIRS {
        let mut files = Vec::new();
        walk(Path::new(dir), &mut files);
        for path in files {
            scan_file(&path, &mut violations);
        }
    }
    report(&violations);
    if violations.is_empty() {
        println!("direction: ok");
    } else {
        exit(1);
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn scan_file(path: &Path, violations: &mut Vec<String>) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    let norm = path.to_string_lossy().replace('\\', "/");
    for (i, raw) in content.lines().enumerate() {
        for needle in banned_in(strip_comment(raw)) {
            violations.push(format!(
                "WRONG_DIRECTION {}:{} `{}` — sim modules must not depend on render/editor/wgpu/egui",
                norm,
                i + 1,
                needle
            ));
        }
    }
}

/// The banned paths `code` references, each matched as a whole path segment so
/// `crate::renderer_stats` or `my_wgpu_like` never trip it.
fn banned_in(code: &str) -> Vec<&'static str> {
    BANNED
        .iter()
        .copied()
        .filter(|needle| {
            code.match_indices(needle).any(|(at, _)| {
                let before = code[..at].chars().next_back();
                let after = code[at + needle.len()..].chars().next();
                !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
            })
        })
        .collect()
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Drop a trailing `//` line comment so doc prose ("never wgpu/egui") doesn't trip
/// the guard. Not string-literal aware — adequate for a coarse source scan.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn report(violations: &[String]) {
    let mut body = if violations.is_empty() {
        String::from("direction: ok\n")
    } else {
        String::from("direction: FAILED\n")
    };
    for v in violations {
        body.push_str(v);
        body.push('\n');
    }
    fs::create_dir_all(".lint").ok();
    fs::write(REPORT, &body).ok();
    if !violations.is_empty() {
        eprint!("{}", body);
        eprintln!(
            "\n{} sim → display-layer reference(s). Move the type to the sim side \
             (components / scene / core) and let render/editor import it instead.",
            violations.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_render_editor_and_gpu_ui_crates() {
        assert_eq!(
            banned_in("use crate::render::Camera;"),
            vec!["crate::render"]
        );
        assert_eq!(
            banned_in("use crate::editor::EditorState;"),
            vec!["crate::editor"]
        );
        assert_eq!(banned_in("let f: wgpu::TextureFormat;"), vec!["wgpu"]);
        assert_eq!(banned_in("use egui::Ui;"), vec!["egui"]);
    }

    #[test]
    fn whole_segments_only() {
        assert!(banned_in("use crate::renderer_stats::X;").is_empty());
        assert!(banned_in("let my_wgpu_like = 1;").is_empty());
        assert!(banned_in("use crate::scene::render_order;").is_empty());
    }

    #[test]
    fn comments_are_ignored() {
        assert!(banned_in(strip_comment("//! never wgpu/egui")).is_empty());
        assert!(banned_in(strip_comment("let x = 1; // crate::render::Camera")).is_empty());
    }

    #[test]
    fn guards_the_sim_and_its_data_modules() {
        assert!(SIM_DIRS.contains(&"src/scene"));
        assert!(SIM_DIRS.contains(&"src/components"));
        assert!(!SIM_DIRS.contains(&"src/render"));
        assert!(!SIM_DIRS.contains(&"src/api"));
    }
}
