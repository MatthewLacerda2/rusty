//! tools/lint — project size gate (the custom part clippy can't do).
//!
//! Measures each `.rs` and `.py` file and fails on any over its cap; nothing is
//! grandfathered (#926). Function-length and code-smell checks are clippy's job
//! (see clippy.toml); without a flag this binary enforces the per-file / per-test-file
//! size rules, and the flags below run the other gates. Dependency-free, std-only.
//!
//! Usage:
//!   cargo run --manifest-path tools/lint/Cargo.toml                  # scan every size root
//!   cargo run --manifest-path tools/lint/Cargo.toml -- a.rs b.py     # check just these files
//!
//! Output: result to stdout and `.lint/report.txt`; exit code 1 on any violation.

mod components;
mod dead_pub;
mod determinism;
mod direction;
mod layers;
mod parity;
mod rules;
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

    // `--layers` runs the module-layering lint (#724): every `crate::<module>`
    // reference must match the declared table in `layers/table.rs`.
    if args.iter().any(|a| a == "--layers") {
        layers::run();
        return;
    }

    // `--components` runs the 4-axis component-completeness gate (#81): every
    // first-class component must have an Entity field, an Add Component entry, an
    // inspector card, and an API namespace, or a written waiver (`waivers.rs`).
    if args.iter().any(|a| a == "--components") {
        components::run();
        return;
    }

    // `--parity` runs the editor↔shared-op parity gate (#287): every *migrated*
    // first-class component's inspector card must route its mutations through a
    // shared `scene::authoring` op (no direct `&mut entity.<field>`).
    if args.iter().any(|a| a == "--parity") {
        parity::run();
        return;
    }

    // `--rules` runs every rule built on the shared `Finding` + inline-allow
    // mechanism (#934): `prefix-group`, `markers`, `skip`, `ignored`, and the ≲10-files signal.
    if args.iter().any(|a| a == "--rules") {
        rules::run();
        return;
    }

    // `--dead-pub` prints the dead-pub signal (#1003): `pub` items nothing uses,
    // and items only tests use. A report, never a gate.
    if args.iter().any(|a| a == "--dead-pub") {
        dead_pub::run();
        return;
    }

    size::run(&args);
}
