//! Component-completeness gate (#81).
//!
//! Every first-class component must satisfy four axes that deliberately live in layers
//! which don't depend on each other:
//!   1. a field on `Entity` (`src/components/entity.rs`) — the discovery source,
//!   2. an Add Component entry (`src/editor/inspector/components/add/`),
//!      guarding on absence through the #344 accessor facade,
//!   3. an inspector card (some `src/editor/inspector/components/*.rs`) writing
//!      through the facade's `_mut`/`set_` accessors,
//!   4. an API namespace (`src/api/<x>.rs` + registration in `src/api/mod.rs`, and a
//!      mention in `docs/api/`).
//!
//! Components are DISCOVERED from `Entity`'s `Option<…Component>` fields, so a new
//! component can't dodge the gate. Axis 1 is the discovery source (always present);
//! the other three are checked here by scanning the source. Today's incomplete
//! components are grandfathered in `tools/lint/components_baseline.txt` — a burn-down
//! list of `component axis` lines (#82) — with the particle system deliberately
//! excluded so it is the gate's first fully-green component.
//!
//! Std-only, like the size gate and the determinism guard. The scan is coarse
//! (substring matches on source), matching the existing lint philosophy.
//!
//! ## Waivers (#82)
//! A few axes are *intentionally* unmet because closing them mechanically would
//! fragment the engine's one stable API surface
//! (`Transform`/`Input`/`Time`/`Physics`/`Scene`/`Animator`/`Nav`/
//! `Camera`/`Material`/…). Those live in [`waivers::WAIVERS`] — an auditable, in-code list
//! of `(component, axis, rationale)` rows. A waived axis counts as satisfied, but
//! unlike the burn-down baseline each waiver carries its written justification
//! right here in the gate, so the decision is reviewable in `git` and can never
//! be a silent skip. The baseline file is the burn-down list for axes we still
//! intend to close; [`waivers::WAIVERS`] is for axes deliberately served by a shared
//! namespace that we will not re-implement standalone.
//!
//! Layout: `discover` (the Entity-field scan), `axes` (the three checkable axes),
//! `waivers` (the deliberate-waiver table); this module runs them and reports.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --components`

use std::fs;
use std::process::exit;

mod axes;
mod discover;
mod waivers;

use axes::{
    add_menu_src, api_stems, editor_blob, has_add_menu, has_api, has_inspector, read, read_md_dir,
};
pub(crate) use discover::discover;
use waivers::waived;

/// The Add Component menu: a module directory, every `.rs` under it.
const ADD_MENU: &str = "src/editor/inspector/components/add";
const API_MOD: &str = "src/api/mod.rs";
const DOCS: &str = "docs/api";
const BASELINE: &str = "tools/lint/components_baseline.txt";
const REPORT: &str = ".lint/report.txt";

/// The checkable axes (axis 1, the `Entity` field, is the discovery source).
const AXES: &[&str] = &["add_menu", "inspector", "api"];

/// Entry point: discover every component and fail on any unbaselined missing axis.
pub fn run() {
    let components = discover();
    let add_src = add_menu_src();
    let editor_blob = editor_blob();
    let api_stems = api_stems();
    let api_mod = read(API_MOD);
    let docs = read_md_dir(DOCS).to_lowercase();
    let baseline = load_baseline();

    let mut violations = Vec::new();
    for field in &components {
        for axis in AXES {
            let ok = match *axis {
                "add_menu" => has_add_menu(field, &add_src),
                "inspector" => has_inspector(field, &editor_blob),
                "api" => has_api(field, &api_stems, &api_mod, &docs),
                _ => true,
            };
            if !ok && !waived(field, axis) && !baselined(field, axis, &baseline) {
                violations.push(format!(
                    "INCOMPLETE_COMPONENT `{field}` missing `{axis}` axis"
                ));
            }
        }
    }
    report(&violations);
    if violations.is_empty() {
        println!("components: ok");
    } else {
        exit(1);
    }
}

/// Baseline entries are `component axis` pairs (whitespace-separated); `#` comments
/// and blank lines are ignored.
fn load_baseline() -> Vec<(String, String)> {
    read(BASELINE)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once(char::is_whitespace))
        .map(|(c, a)| (c.trim().to_string(), a.trim().to_string()))
        .collect()
}

fn baselined(field: &str, axis: &str, baseline: &[(String, String)]) -> bool {
    baseline.iter().any(|(c, a)| c == field && a == axis)
}

fn report(violations: &[String]) {
    let mut body = if violations.is_empty() {
        String::from("components: ok\n")
    } else {
        String::from("components: FAILED\n")
    };
    for v in violations {
        body.push_str(v);
        body.push('\n');
    }
    fs::create_dir_all(".lint").ok();
    fs::write(REPORT, &body).ok();
    if !violations.is_empty() {
        eprint!("{body}");
        eprintln!(
            "\n{} incomplete component axis(es). Complete the axis, or add a \
             `component axis` line to {BASELINE} (burn-down only — see docs/linting.md).",
            violations.len()
        );
    }
}
