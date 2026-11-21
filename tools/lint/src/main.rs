//! The lint tool (`tools/lint`): the project size gate (the custom part clippy
//! can't do).
//!
//! Measures each `.rs`, `.py`, `.lua` and `.wgsl` file and fails on any over its cap;
//! nothing is grandfathered (#926); it holds the agent docs to their shape too
//! (`agent_docs`). Function-length and code-smell checks are clippy's job
//! (see clippy.toml); without a flag this binary enforces the per-file / per-test-file
//! size rules, and each flag of `gates::GATES` runs another gate. Dependency-free,
//! std-only.
//!
//! Usage:
//!   cargo run --manifest-path tools/lint/Cargo.toml                  # scan every size root
//!   cargo run --manifest-path tools/lint/Cargo.toml -- a.rs b.py     # check just these files
//!
//! Output: result to stdout and `.lint/report.txt`; exit code 1 on any violation.

mod agent_docs;
mod code;
mod comments;
mod components;
mod dead_pub;
mod determinism;
mod gates;
mod layers;
mod modules;
mod parity;
mod repo;
mod rules;
mod size;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Every gate is a row of `gates::GATES`, chosen by its flag (`docs/linting.md`
    // says what each holds); with no flag, the size gate. Two flags are refused.
    let check = gates::select(&args).unwrap_or_else(|why| {
        eprintln!("{why}");
        std::process::exit(2);
    });

    // `--dead-pub` prints the dead-pub signal (#1003): `pub` items nothing uses,
    // and items only tests use. A report, never a gate, so not a row of
    // `gates::GATES`: it builds the workspace twice, and fails only when it cannot.
    if args.iter().any(|a| a == gates::DEAD_PUB) {
        dead_pub::run();
        return;
    }
    // `--comment-only [<rev>]` proves the branch changes only comments: a tool
    // mode a cleanup branch runs, not a gate, so not a row either.
    if let Some(at) = args.iter().position(|a| a == comments::proof::FLAG) {
        let rev = args
            .get(at + 1)
            .map_or(comments::proof::DEFAULT_BASE, String::as_str);
        gates::report(&comments::proof::verdict(std::path::Path::new(""), rev));
        return;
    }
    gates::report(&check(std::path::Path::new(""), &args));
}
