use crate::gates::Verdict;
use std::fs;
use std::path::{Path, PathBuf};

use crate::components;

const CARDS_DIR: &str = "src/editor/inspector/components";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Card {
    None,
    Routed,
    Direct,
}

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

fn classify_all(field: &str, sources: &[String]) -> Card {
    let cards = sources.iter().map(|src| classify(field, src));
    cards.fold(Card::None, |acc, c| match (acc, c) {
        (Card::Direct, _) | (_, Card::Direct) => Card::Direct,
        (Card::Routed, _) | (_, Card::Routed) => Card::Routed,
        _ => Card::None,
    })
}

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

        // A member crate spells the ops through `rusty_edit`; the base's builders are no ops.
        let member = "use rusty_edit::authoring::light as light_ops;\n\
                      if let Some(mut c) = world.light_mut(id) { light_ops::set_color(&mut c, rgb); }";
        assert_eq!(classify("light", member), Card::Routed);
        let base = "use rusty_scene::create::default_light;\n\
                    let mut l = world.light_mut(id).unwrap(); *l = default_light();";
        assert_eq!(classify("light", base), Card::Direct);

        let read_only = "if world.has_mesh(id) { world.set_mesh(id, None); }";
        assert_eq!(classify("mesh", read_only), Card::Routed);
    }

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

    #[test]
    fn classify_reports_no_card() {
        assert_eq!(
            classify("rigidbody", "world.set_light(id, None);"),
            Card::None
        );
    }

    #[test]
    fn enforcement_table() {
        assert!(violation("light", Card::Direct).is_some());
        assert!(violation("material", Card::Routed).is_none());
        assert!(violation("mesh", Card::None).is_none());
    }

    #[test]
    fn material_regression_to_direct_is_flagged() {
        let regressed = "let mut mat = world.material_mut(id).unwrap();\n\
                         mat.material = \"other\".to_string();";
        assert_eq!(classify("material", regressed), Card::Direct);
        assert!(violation("material", Card::Direct).is_some());
    }
}
