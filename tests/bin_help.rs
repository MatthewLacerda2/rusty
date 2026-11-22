//! Every engine binary answers `--help`/`-h` with usage and exits 0 before opening a project,
//! window or session. The table is held equal to `Cargo.toml`'s `[[bin]]` names. Gated on `dev`.

use std::process::{Command, Output, Stdio};
use std::time::Duration;

/// Poll count for a running binary; counted, not timed: tests read no wall clock.
const POLL: Duration = Duration::from_millis(50);
const POLLS: u32 = 1200;

fn binaries() -> Vec<(&'static str, Command)> {
    use crate::spawn::engine_bin;
    vec![
        ("rusty", engine_bin!("rusty")),
        ("player", engine_bin!("player")),
        ("play", engine_bin!("play")),
        ("export", engine_bin!("export")),
        ("session", engine_bin!("session")),
        ("session-mcp", engine_bin!("session-mcp")),
        ("game", engine_bin!("game")),
        ("editor-capture", engine_bin!("editor-capture")),
        ("bench-scenes", engine_bin!("bench-scenes")),
    ]
}

fn run(name: &str, mut command: Command, args: &[&str]) -> Output {
    let mut child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("{name}: cannot start: {e}"));
    let mut polls = 0;
    while child.try_wait().expect("wait").is_none() {
        polls += 1;
        if polls > POLLS {
            child.kill().ok();
            panic!(
                "`{name} {}` did not return in about a minute",
                args.join(" ")
            );
        }
        std::thread::sleep(POLL);
    }
    child.wait_with_output().expect("output")
}

#[test]
fn every_binary_prints_its_usage_for_help_and_exits() {
    for flag in ["--help", "-h"] {
        for (name, command) in binaries() {
            let out = run(name, command, &[flag]);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(
                out.status.success(),
                "`{name} {flag}`: {}\n{stderr}",
                out.status
            );
            assert!(
                stdout.starts_with(&format!("usage: {name} ")),
                "`{name} {flag}` printed no usage:\n{stdout}"
            );
        }
    }
}

#[test]
fn the_editor_refuses_an_argument_it_does_not_know() {
    let out = run(
        "rusty",
        crate::spawn::engine_bin!("rusty"),
        &["--no-such-flag"],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("unknown argument --no-such-flag"),
        "{stderr}"
    );
    assert!(stderr.contains("usage: rusty"), "{stderr}");
}

#[test]
fn the_table_names_every_binary_cargo_builds() {
    let manifest = include_str!("../Cargo.toml");
    let mut declared = Vec::new();
    let mut in_bin = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_bin = line == "[[bin]]";
        } else if let Some(name) = line.strip_prefix("name = ").filter(|_| in_bin) {
            declared.push(name.trim_matches('"'));
        }
    }
    let mut tested: Vec<&str> = binaries().into_iter().map(|(name, _)| name).collect();
    declared.sort_unstable();
    tested.sort_unstable();
    assert_eq!(
        declared, tested,
        "every `[[bin]]` in Cargo.toml needs a row in `bin_help::binaries`"
    );
}
