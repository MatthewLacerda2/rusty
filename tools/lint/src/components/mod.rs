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

const ADD_MENU: &str = "src/editor/inspector/components/add";
const API_MOD: &str = "src/script/api/mod.rs";
const DOCS: &str = "docs/api";

/// Axes 2 to 4: axis 1, the registry row, is where components are discovered.
const AXES: &[&str] = &["add_menu", "inspector", "api"];

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

pub(crate) fn unreadable_registry(gate: &str, why: String) -> Verdict {
    Verdict::listed(gate, vec![why], |_| {
        "\nThe component registry could not be read, so no component was checked: \
         point `components/discover.rs` at the registry's `first_class!` rows \
         (docs/linting.md).\n"
            .to_string()
    })
}

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
