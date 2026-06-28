//! Editor↔shared-op parity gate (#287).
//!
//! A first-class component is authored from two places that must never drift: the
//! editor's inspector card and the Lua API namespace. PR-1 made them siblings over a
//! shared `scene::authoring` op (the editor card AND the Lua binding both call it, so
//! the write + its validation live once). This gate keeps *migrated* cards honest:
//! a card that has been routed through a shared op must stay routed — reintroducing a
//! direct field mutation fails the build.
//!
//! Components are DISCOVERED the same way the completeness gate does — from `Entity`'s
//! `Option<…Component>` fields (`components::discover`) — so this is the same
//! non-fragile enumeration, not a fresh attempt to auto-find arbitrary editor
//! capabilities. For each discovered component that has an inspector card, the card is
//! classified:
//!   * **direct** — the card takes a mutable borrow of the component (`&mut
//!     entity.<field>`) and writes its fields through egui widgets, OR
//!   * **routed** — the card reads fields immutably and routes every write through a
//!     shared `authoring::…` op (like the migrated `material` card). Detaching the
//!     reference on remove (`entity.<field> = None`) is NOT a field mutation, so a
//!     routed card may still do it.
//!
//! The signal is `&mut entity.<field>`: every un-migrated card takes that borrow to
//! edit fields, while the migrated material card reads via `entity.material.as_ref()`
//! and only does `entity.material = None` on remove — so the marker cleanly separates
//! them with a very low false-positive rate.
//!
//! ## Enforcement (teeth + burn-down)
//!   * A **direct** card NOT in `parity_baseline.txt` → violation (drift: a direct
//!     mutation was added/kept without grandfathering it).
//!   * A component in the baseline whose card is actually **routed** → violation
//!     (stale entry: you migrated the card but left it baselined — remove the line).
//!   * A **routed** card NOT in the baseline → ok (the migrated capability; `material`
//!     is the first one, and this is the gate's teeth — reverting it to direct fails).
//!   * A **direct** card in the baseline → ok (grandfathered, awaiting migration).
//!   * A component with no inspector card → skipped (not a parity concern here).
//!
//! Std-only and coarse (substring scans), matching the size/determinism/components
//! gates' philosophy.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --parity`

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use crate::components;

/// The inspector cards live here (one module per first-class component). Scoping the
/// scan to this directory is exact: a grep confirms no other editor file takes a
/// `&mut entity.<component>` borrow, so the cards are the sole authoring surface.
const CARDS_DIR: &str = "src/editor/inspector/components";
const BASELINE: &str = "tools/lint/parity_baseline.txt";
const REPORT: &str = ".lint/report.txt";

/// How a component's inspector card mutates the component.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Card {
    /// No card references this component (skip — not a parity concern).
    None,
    /// Routes every write through a shared `authoring::…` op (no `&mut` to the field).
    Routed,
    /// Takes `&mut entity.<field>` and writes fields directly via egui widgets.
    Direct,
}

/// Entry point: classify every discovered component's card and fail on drift or a
/// stale baseline entry.
pub fn run() {
    let components = components::discover();
    let blob = cards_blob();
    let baseline = load_baseline();

    let mut violations = Vec::new();
    for field in &components {
        let card = classify(field, &blob);
        let listed = baseline.iter().any(|b| b == field);
        if let Some(v) = violation(field, card, listed) {
            violations.push(v);
        }
    }
    report(&violations);
    if violations.is_empty() {
        println!("parity: ok");
    } else {
        exit(1);
    }
}

/// The enforcement table (pure, so it is exhaustively testable): given a card's
/// classification and whether the component is baselined, return a violation or `None`.
fn violation(field: &str, card: Card, baselined: bool) -> Option<String> {
    match (card, baselined) {
        // Drift: a direct card must be grandfathered, or it is unaccounted-for.
        (Card::Direct, false) => Some(format!(
            "PARITY_DRIFT `{field}` card mutates the component directly \
             (`&mut entity.{field}`) but is not in {BASELINE} — route it through a \
             `scene::authoring` op, or (exceptionally) baseline it with justification"
        )),
        // Stale: a migrated (routed) card must not stay on the burn-down list.
        (Card::Routed, true) => Some(format!(
            "PARITY_STALE_BASELINE `{field}` card is routed through a shared op but is \
             still listed in {BASELINE} — remove the line (burn-down hygiene)"
        )),
        // direct+baselined = grandfathered; routed+unbaselined = migrated; no card = skip.
        _ => None,
    }
}

/// Classify `field`'s inspector card from the concatenated cards source.
///
/// A card "exists" when some card source references `entity.<field>` (mirrors the
/// completeness gate's inspector axis). It is **direct** when it takes a mutable
/// borrow of the component (`&mut entity.<field>`), else **routed**.
fn classify(field: &str, blob: &str) -> Card {
    if !blob.contains(&format!("entity.{field}")) {
        return Card::None;
    }
    if blob.contains(&format!("&mut entity.{field}")) {
        Card::Direct
    } else {
        Card::Routed
    }
}

/// Concatenate every inspector-card source under [`CARDS_DIR`].
fn cards_blob() -> String {
    let mut files = Vec::new();
    walk(Path::new(CARDS_DIR), &mut files);
    files.iter().map(read).collect::<Vec<_>>().join("\n")
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

fn read<P: AsRef<Path>>(path: P) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// Baseline entries are bare component names (one per line); `#` comments and blank
/// lines are ignored — same format as `components_baseline.txt`.
fn load_baseline() -> Vec<String> {
    read(BASELINE)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

fn report(violations: &[String]) {
    let mut body = if violations.is_empty() {
        String::from("parity: ok\n")
    } else {
        String::from("parity: FAILED\n")
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
            "\n{} editor-card parity issue(s). Route the card through a shared \
             `scene::authoring` op (and drop its {BASELINE} line), or fix the \
             baseline — see docs/linting.md.",
            violations.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A routed snippet (calls `authoring::…`, no `&mut` to the component) classifies
    /// as routed; a direct snippet (`&mut entity.light` then field writes) as direct.
    #[test]
    fn classify_separates_routed_from_direct() {
        let routed = "let key = entity.material.as_ref();\n\
                      mat_ops::set_metallic(materials, key, metallic);\n\
                      entity.material = None;";
        assert_eq!(classify("material", routed), Card::Routed);

        let direct = "let Some(light) = &mut entity.light else { return; };\n\
                      light.color = Vec3::new(r, g, b);";
        assert_eq!(classify("light", direct), Card::Direct);
    }

    /// A component no card references is skipped (not a parity concern).
    #[test]
    fn classify_reports_no_card() {
        assert_eq!(classify("rigidbody", "entity.light = None;"), Card::None);
    }

    /// The enforcement table: drift, stale baseline, and the two ok cases.
    #[test]
    fn enforcement_table() {
        // Direct + not baselined → drift violation.
        assert!(violation("light", Card::Direct, false).is_some());
        // Direct + baselined → grandfathered, ok.
        assert!(violation("light", Card::Direct, true).is_none());
        // Routed + still baselined → stale-baseline violation.
        assert!(violation("material", Card::Routed, true).is_some());
        // Routed + not baselined → the migrated capability, ok.
        assert!(violation("material", Card::Routed, false).is_none());
        // No card → never a violation, baselined or not.
        assert!(violation("mesh", Card::None, false).is_none());
        assert!(violation("mesh", Card::None, true).is_none());
    }

    /// The teeth: reintroducing a direct `MaterialAsset` mutation into the (un-
    /// baselined) material card is flagged. We model "material went direct" as a card
    /// that takes `&mut entity.material`; with material absent from the baseline, that
    /// is a drift violation — exactly what stops the migration from being reverted.
    #[test]
    fn material_regression_to_direct_is_flagged() {
        let regressed = "let Some(mat) = &mut entity.material else { return; };\n\
                         mat.metallic = 1.0;";
        assert_eq!(classify("material", regressed), Card::Direct);
        assert!(violation("material", Card::Direct, false).is_some());
    }
}
