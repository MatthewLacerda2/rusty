//! tools/lint — project size gate (the custom part clippy can't do).
//!
//! Counts per-file lines and fails on any file over its cap. Function-length and
//! code-smell checks are clippy's job (see clippy.toml); this binary only enforces
//! the per-file / per-test-file size rules. Dependency-free, std-only.
//!
//! Usage:
//!   cargo run --manifest-path tools/lint/Cargo.toml            # scan size::SCAN_ROOTS, minus baseline
//!   cargo run --manifest-path tools/lint/Cargo.toml -- a.rs b.rs   # check just these files
//!
//! Output: result to stdout and `.lint/report.txt`; exit code 1 on any violation.

mod components;
mod determinism;
mod direction;
mod parity;
mod size;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // `--determinism` runs the sim-purity guard instead of the size gate: it fails
    // if wall-clock / unseeded-RNG calls leak into the deterministic sim modules.
    if args.iter().any(|a| a == "--determinism") {
        determinism::run();
        return;
    }

    // `--direction` runs the dependency-direction guard (#494): sim modules must not
    // reference `crate::render`, `crate::editor`, `wgpu` or `egui`.
    if args.iter().any(|a| a == "--direction") {
        direction::run();
        return;
    }

    // `--components` runs the 4-axis component-completeness gate (#81): every
    // first-class component must have an Entity field, an Add Component entry, an
    // inspector card, and an API namespace, or be grandfathered in the baseline.
    if args.iter().any(|a| a == "--components") {
        components::run();
        return;
    }

    // `--parity` runs the editor↔shared-op parity gate (#287): every *migrated*
    // first-class component's inspector card must route its mutations through a
    // shared `scene::authoring` op (no direct `&mut entity.<field>`); cards not yet
    // migrated are grandfathered in `parity_baseline.txt` (a burn-down list).
    if args.iter().any(|a| a == "--parity") {
        parity::run();
        return;
    }

    size::run(&args);
}
