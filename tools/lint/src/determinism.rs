//! Determinism guard — protects the headless harness's bit-determinism.
//!
//! The simulation must be a pure function of (seed, inputs, fixed dt). Wall-clock
//! reads and unseeded RNG break replayability, so they are BANNED from the sim
//! modules and only allowed in the platform/display layer (`main.rs`, `shell/`,
//! `render/`, `editor/`, `dev/`, `preview/`, `bin/`), which legitimately needs real
//! time and randomness.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --determinism`
//! Exit code 1 on any violation; report mirrored to `.lint/report.txt`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

/// Directories whose `.rs` files are sim code and must stay deterministic: the four
/// sim trees and the in-game UI layout (#417); `api`, the Rust side of every Lua call
/// a script makes inside `FixedUpdate`/`Update` (#723, the twin of #443's Lua-side
/// ban); the data the sim runs on (`scene`, `components`, `ecs`, `core`, `time`,
/// `asset`); `procgen` and `shadergen`, which the scene's materials and `Shader.Bake`
/// reach; and `audio`, whose maestro is a `Resource` stepped by sim time — its kira
/// device thread is fed mixes, it never reads a clock, so it stays inside the scan.
///
/// The synthesiser behind `Sound.*` is no longer here: it is scorsese's zimmer
/// crate (#413), whose byte-identical promise scorsese guards itself.
///
/// A listed directory that no longer exists is a violation, so a rename can't
/// silently drop a module out of the scan.
const SIM_DIRS: &[&str] = &[
    "src/app",
    "src/scripting",
    "src/physics",
    "src/navigation",
    "src/ui",
    "src/api",
    "src/scene",
    "src/components",
    "src/ecs",
    "src/core",
    "src/time",
    "src/asset",
    "src/procgen",
    "src/shadergen",
    "src/audio",
];

/// Banned call fragments. Matched as substrings on non-comment source.
/// `rand::random` is the unseeded global RNG; seeded generators (`StdRng::from_seed`,
/// a threaded-through `&mut impl Rng`) are fine and not matched here.
const BANNED: &[&str] = &["Instant::now", "SystemTime", "rand::random"];

const REPORT: &str = ".lint/report.txt";

/// Entry point: scan the sim modules and fail on any banned wall-clock / RNG use.
pub fn run() {
    let mut violations = Vec::new();
    for dir in SIM_DIRS {
        if !Path::new(dir).is_dir() {
            violations.push(format!(
                "MISSING_DIR {dir} — listed in SIM_DIRS but absent; update the list"
            ));
            continue;
        }
        let mut files = Vec::new();
        walk(Path::new(dir), &mut files);
        for path in files {
            scan_file(&path, &mut violations);
        }
    }
    report(&violations);
    if violations.is_empty() {
        println!("determinism: ok");
    } else {
        exit(1);
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().map_or(false, |e| e == "rs") {
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
        let code = strip_comment(raw);
        for needle in BANNED {
            if code.contains(needle) {
                violations.push(format!(
                    "NON_DETERMINISTIC {}:{} `{}` — wall-clock/RNG is banned in sim modules",
                    norm,
                    i + 1,
                    needle
                ));
            }
        }
    }
}

/// Drop a trailing `//` line comment so commented-out code / doc mentions don't
/// trip the guard. Not string-literal aware — adequate for a coarse source scan.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn report(violations: &[String]) {
    let mut body = if violations.is_empty() {
        String::from("determinism: ok\n")
    } else {
        String::from("determinism: FAILED\n")
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
            "\n{} wall-clock/RNG call(s) in deterministic sim modules. Move them to the \
             platform layer (main.rs / render / dev) or thread a seeded source.",
            violations.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_comment_drops_trailing_line_comment() {
        assert_eq!(strip_comment("let x = 1; // Instant::now"), "let x = 1; ");
        assert_eq!(strip_comment("no comment here"), "no comment here");
    }

    #[test]
    fn banned_list_covers_the_three_sources() {
        assert!(BANNED.contains(&"Instant::now"));
        assert!(BANNED.contains(&"SystemTime"));
        assert!(BANNED.contains(&"rand::random"));
    }

    #[test]
    fn sim_dirs_are_pinned() {
        // Dropping a module off the scan must be a deliberate, reviewed edit (#723).
        let mut dirs = SIM_DIRS.to_vec();
        dirs.sort_unstable();
        let mut want = [
            "src/api",
            "src/app",
            "src/asset",
            "src/audio",
            "src/components",
            "src/core",
            "src/ecs",
            "src/navigation",
            "src/physics",
            "src/procgen",
            "src/scene",
            "src/scripting",
            "src/shadergen",
            "src/time",
            "src/ui",
        ];
        want.sort_unstable();
        assert_eq!(dirs, want);
    }

    #[test]
    fn sim_dirs_all_exist() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for dir in SIM_DIRS {
            assert!(root.join(dir).is_dir(), "{dir} is listed but missing");
        }
    }
}
