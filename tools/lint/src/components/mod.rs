//! Component-completeness gate (#81).
//!
//! Every first-class component must satisfy four axes that deliberately live in layers
//! which don't depend on each other:
//!   1. a row of the registry (`first_class!`, located under `crates/components/src/`,
//!      #2023) — the discovery source, from which `Entity`'s field is generated,
//!   2. an Add Component entry (`src/editor/inspector/components/add/`),
//!      guarding on absence through the #344 accessor facade,
//!   3. an inspector card (some `src/editor/inspector/components/*.rs`) writing
//!      through the facade's `_mut`/`set_` accessors,
//!   4. an API namespace (`src/script/api/<x>.rs` + registration in `src/script/api/mod.rs`, and a
//!      mention in `docs/api/`).
//!
//! Components are DISCOVERED from the registry's rows, so a new
//! component can't dodge the gate. Axis 1 is the discovery source (always present);
//! the other three are checked here by scanning the source. Nothing is grandfathered
//! (#926): the burn-down list of incomplete axes emptied under #82 and was retired, so
//! a missing axis is either closed or a written waiver below.
//!
//! Std-only, like the size gate and the determinism guard. The scan is coarse
//! (substring matches on source), matching the existing lint philosophy.
//!
//! ## Waivers (#82)
//! A few axes are *intentionally* unmet because closing them mechanically would
//! fragment the engine's one stable API surface
//! (`Transform`/`Input`/`Time`/`Physics`/`Scene`/`Animator`/`Nav`/
//! `Camera`/`Material`/…). Those live in [`waivers::WAIVERS`] — an auditable, in-code list
//! of `(component, axis, rationale)` rows. A waived axis counts as satisfied, and
//! each waiver carries its written justification right here in the gate, so the
//! decision is reviewable in `git` and can never be a silent skip. It is for axes
//! deliberately served by a shared namespace that we will not re-implement
//! standalone, never for an axis not closed yet.
//!
//! Layout: `discover` (the registry-row scan), `kinds` (the generated menu's guards), `axes` (the three checkable axes),
//! `waivers` (the deliberate-waiver table); this module runs them and reports.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --components`

use crate::gates::Verdict;
use std::path::Path;

mod axes;
mod discover;
mod kinds;
mod waivers;

use axes::{
    add_menu_src, api_stems, editor_blob, has_add_menu, has_api, has_inspector, read, read_md_dir,
};
pub(crate) use discover::discover_in;
use waivers::{stale_waivers, waived};

/// The Add Component menu: a module directory, every `.rs` under it.
const ADD_MENU: &str = "src/editor/inspector/components/add";
const API_MOD: &str = "src/script/api/mod.rs";
const DOCS: &str = "docs/api";

/// The checkable axes (axis 1, the registry row, is the discovery source).
const AXES: &[&str] = &["add_menu", "inspector", "api"];

/// The gate over the repository at `base`: every component discovered, each
/// unwaived missing axis a finding, and so is a registry it cannot read.
pub fn verdict(base: &Path, _: &[String]) -> Verdict {
    match discover_in(base) {
        Ok(components) => Verdict::listed("components", incomplete(base, &components), |n| {
            format!(
                "\n{n} component gate issue(s). Complete the axis, or waive it in \
                 `waivers::WAIVERS` with its reason (a stale waiver: delete its row) — \
                 see docs/linting.md.\n"
            )
        }),
        Err(why) => unreadable_registry("components", why),
    }
}

/// The verdict of a gate that could not read the registry it discovers
/// components from: that one finding, and the fix for it alone.
pub(crate) fn unreadable_registry(gate: &str, why: String) -> Verdict {
    Verdict::listed(gate, vec![why], |_| {
        "\nThe component registry could not be read, so no component was checked: \
         point `components/discover.rs` at the registry's `first_class!` rows \
         (docs/linting.md).\n"
            .to_string()
    })
}

/// Every unwaived missing axis of `components`, and every stale waiver.
fn incomplete(base: &Path, components: &[String]) -> Vec<String> {
    let registry = discover::registry_source(&crate::modules::real(base, discover::COMPONENTS_DIR))
        .unwrap_or_default();
    let add_src = kinds::with_kind_guards(&add_menu_src(base), &registry);
    let editor_blob = editor_blob(base);
    let api_stems = api_stems(base);
    let api_mod = read(crate::modules::real(base, API_MOD));
    let docs = read_md_dir(&base.join(DOCS)).to_lowercase();

    let present = |field: &str, axis: &str| match axis {
        "add_menu" => has_add_menu(field, &add_src),
        "inspector" => has_inspector(field, &editor_blob),
        "api" => has_api(field, &api_stems, &api_mod, &docs),
        _ => true,
    };
    let mut violations = Vec::new();
    for field in components {
        for axis in AXES {
            if !present(field, axis) && !waived(field, axis) {
                violations.push(format!(
                    "INCOMPLETE_COMPONENT `{field}` missing `{axis}` axis"
                ));
            }
        }
    }
    violations.extend(stale_waivers(components, present));
    violations
}
