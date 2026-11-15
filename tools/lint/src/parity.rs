//! Editor↔shared-op parity gate (#287).
//!
//! A first-class component is authored from two places that must never drift: the
//! editor's inspector card and the Lua API namespace. PR-1 made them siblings over a
//! shared `edit::authoring` op (the editor card AND the Lua binding both call it, so
//! the write + its validation live once). This gate keeps *migrated* cards honest:
//! a card that has been routed through a shared op must stay routed — reintroducing a
//! direct field mutation fails the build.
//!
//! Components are DISCOVERED the same way the completeness gate does — from the
//! registry's `first_class!` rows (`components::discover_in`) — so this is the same
//! non-fragile enumeration, not a fresh attempt to auto-find arbitrary editor
//! capabilities. For each discovered component that has an inspector card, the card is
//! classified:
//!   * **direct** — the card takes the component's mutable accessor and writes
//!     fields through egui widgets without routing through its authoring ops, OR
//!   * **routed** — the card reads a snapshot immutably and routes every write
//!     through a shared `authoring::…` op. Detaching on remove
//!     (`set_<field>(id, None)`) is NOT a field mutation, so a routed card may
//!     still do it.
//!
//! Post-#344 the cards reach components through the accessor facade, so the signal
//! is facade-shaped: a card file that takes a mutable accessor (`.<field>_mut(`)
//! without importing any `edit::authoring` ops is mutating fields directly;
//! a routed card only ever takes the guard to hand `&mut c` into a shared op.
//! Detach on remove (`.set_<field>(id, None)`) is not a field mutation.
//!
//! ## Enforcement
//!   * A **direct** card → violation (drift: a direct mutation was added or kept).
//!   * A **routed** card → ok (every card is routed today; reverting one fails).
//!   * A component with no inspector card → skipped (not a parity concern here).
//!
//! Nothing is grandfathered (#926): the burn-down list this gate once read emptied
//! under #287 and was retired. A card that cannot route yet is a card to fix.
//!
//! Std-only and coarse (substring scans), matching the size/determinism/components
//! gates' philosophy.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --parity`

use crate::gates::Verdict;
use std::fs;
use std::path::{Path, PathBuf};

use crate::components;

/// The inspector cards live here (one module per first-class component). Scoping the
/// scan to this directory is exact: a grep confirms no other editor file takes a
/// component's mutable accessor, so the cards are the sole authoring surface.
const CARDS_DIR: &str = "src/editor/inspector/components";

/// How a component's inspector card mutates the component.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Card {
    /// No card references this component (skip — not a parity concern).
    None,
    /// Routes every write through a shared `authoring::…` op (the mutable accessor
    /// is only ever taken to hand `&mut c` into an op).
    Routed,
    /// Takes the component's mutable accessor without the shared ops in scope —
    /// fields are being written directly via egui widgets.
    Direct,
}

/// The gate over the repository at `base`: every discovered component's card
/// classified, each drift a finding, and so is a registry it cannot read.
pub fn verdict(base: &Path, _: &[String]) -> Verdict {
    let components = match components::discover_in(base) {
        Ok(components) => components,
        Err(why) => return components::unreadable_registry("parity", why),
    };
    let cards = card_sources(base);
    let violations = components
        .iter()
        .filter_map(|field| violation(field, classify_all(field, &cards)))
        .collect();
    Verdict::listed("parity", violations, |n| {
        format!(
            "\n{n} editor-card parity issue(s). Route the card through a shared \
             `edit::authoring` op — see docs/linting.md.\n"
        )
    })
}

/// The enforcement table (pure, so it is exhaustively testable): given a card's
/// classification, return a violation or `None`.
fn violation(field: &str, card: Card) -> Option<String> {
    match card {
        Card::Direct => Some(format!(
            "PARITY_DRIFT `{field}` card mutates the component directly \
             (`{field}_mut` without its `edit::authoring` ops) — route it through a \
             shared op"
        )),
        Card::Routed | Card::None => None,
    }
}

/// Classify `field`'s card across every card source: direct if any file is, else
/// routed if any file touches the component, else no card.
fn classify_all(field: &str, sources: &[String]) -> Card {
    let cards = sources.iter().map(|src| classify(field, src));
    cards.fold(Card::None, |acc, c| match (acc, c) {
        (Card::Direct, _) | (_, Card::Direct) => Card::Direct,
        (Card::Routed, _) | (_, Card::Routed) => Card::Routed,
        _ => Card::None,
    })
}

/// Classify `field`'s inspector card in one card source file.
///
/// A card "exists" when the file touches the component through the #344 facade
/// (mirrors the completeness gate's inspector axis). It is **direct** when it takes
/// the mutable accessor (`.<field>_mut(`) in a file that imports no
/// `edit::authoring` ops — a routed card only takes the guard to hand `&mut c`
/// into a shared op it imported, whatever that op module is called (`nav::agent`
/// for `nav_agent`, `backdrop` for `backdrop_filter`), and a read-only-plus-remove
/// card never takes it at all. Judged per file: the old whole-tree name match
/// misread every card whose ops module isn't named after its field (#959).
fn classify(field: &str, src: &str) -> Card {
    let takes_mut = src.contains(&format!(".{field}_mut("));
    let exists = takes_mut
        || src.contains(&format!(".set_{field}("))
        || src.contains(&format!(".has_{field}("));
    if !exists {
        return Card::None;
    }
    if takes_mut && !src.contains("edit::authoring::") {
        Card::Direct
    } else {
        Card::Routed
    }
}

/// Every inspector-card source under [`CARDS_DIR`], one string per file.
fn card_sources(base: &Path) -> Vec<String> {
    let mut files = Vec::new();
    walk(&crate::modules::real(base, CARDS_DIR), &mut files);
    files.iter().map(read).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A routed snippet (ops imported, guard only feeds the op) classifies as
    /// routed; a direct snippet (mutable accessor, no ops) as direct.
    #[test]
    fn classify_separates_routed_from_direct() {
        let routed = "use crate::edit::authoring::light as light_ops;\n\
                      if let Some(mut c) = world.light_mut(id) {\n\
                          light_ops::set_color(&mut c, rgb);\n\
                      }";
        assert_eq!(classify("light", routed), Card::Routed);

        let direct = "let mut l = world.light_mut(id).unwrap();\n\
                      l.color = Vec3::new(r, g, b);";
        assert_eq!(classify("light", direct), Card::Direct);

        // A member crate spells the ops through the edit crate; the base's own
        // re-exported builders are no ops.
        let member = "use rusty_edit::authoring::light as light_ops;\n\
                      if let Some(mut c) = world.light_mut(id) { light_ops::set_color(&mut c, rgb); }";
        assert_eq!(classify("light", member), Card::Routed);
        let base = "use rusty_scene::create::default_light;\n\
                    let mut l = world.light_mut(id).unwrap(); *l = default_light();";
        assert_eq!(classify("light", base), Card::Direct);

        // Read-only + detach-on-remove (the mesh card's shape) stays routed.
        let read_only = "if world.has_mesh(id) { world.set_mesh(id, None); }";
        assert_eq!(classify("mesh", read_only), Card::Routed);
    }

    /// The ops module need not be named after the field, and a direct file is
    /// not excused by a routed one elsewhere.
    #[test]
    fn routing_is_judged_per_file() {
        let routed = "use crate::edit::authoring::nav::agent as nav_ops;\n\
                      if let Some(mut a) = world.nav_agent_mut(id) { nav_ops::set(&mut a); }";
        assert_eq!(classify("nav_agent", routed), Card::Routed);
        let direct = "let mut a = world.nav_agent_mut(id).unwrap(); a.speed = 2.0;";
        let both = [routed.to_string(), direct.to_string()];
        assert_eq!(classify_all("nav_agent", &both), Card::Direct);
        assert_eq!(classify_all("nav_agent", &both[..1]), Card::Routed);
        assert_eq!(classify_all("nav_agent", &[]), Card::None);
    }

    /// A component no card references is skipped (not a parity concern).
    #[test]
    fn classify_reports_no_card() {
        assert_eq!(
            classify("rigidbody", "world.set_light(id, None);"),
            Card::None
        );
    }

    /// The enforcement table: a direct card fails; routed and absent ones pass.
    #[test]
    fn enforcement_table() {
        assert!(violation("light", Card::Direct).is_some());
        assert!(violation("material", Card::Routed).is_none());
        assert!(violation("mesh", Card::None).is_none());
    }

    /// The teeth: reintroducing a direct `MaterialComponent` mutation into the
    /// material card is flagged. We model "material went direct" as a card that
    /// takes the mutable accessor with no shared ops in scope — a drift violation,
    /// exactly what stops the migration from being reverted.
    #[test]
    fn material_regression_to_direct_is_flagged() {
        let regressed = "let mut mat = world.material_mut(id).unwrap();\n\
                         mat.material = \"other\".to_string();";
        assert_eq!(classify("material", regressed), Card::Direct);
        assert!(violation("material", Card::Direct).is_some());
    }
}
