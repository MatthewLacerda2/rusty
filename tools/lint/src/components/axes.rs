//! The three checkable axes (2–4): Add Component entry, inspector card, and API
//! namespace — coarse substring scans over the editor/API sources and the docs.

use std::fs;
use std::path::{Path, PathBuf};

use super::ADD_MENU;

const EDITOR_DIR: &str = "src/editor";
const API_DIR: &str = "src/api";

/// Axis 2: a standalone Add Component entry guards on absence via the #344
/// accessor facade (`!world.has_<field>(id)`). A component only added as a
/// side-effect of another has no such guard and is reported until it gets its
/// own entry (#82).
pub(super) fn has_add_menu(field: &str, add_src: &str) -> bool {
    add_src.contains(&format!("!world.has_{field}(id)"))
}

/// Axis 3: some editor file other than the Add menu edits the component in a
/// card — through the facade, that is a mutable accessor (`.<field>_mut(`) or a
/// detach/attach write (`.set_<field>(`).
pub(super) fn has_inspector(field: &str, editor_blob: &str) -> bool {
    editor_blob.contains(&format!(".{field}_mut("))
        || editor_blob.contains(&format!(".set_{field}("))
}

/// Axis 4: an API namespace named after the component (its field, or the singular
/// of a plural field) exists, is registered in `api/mod.rs`, and is documented.
pub(super) fn has_api(field: &str, api_stems: &[String], api_mod: &str, docs_lower: &str) -> bool {
    candidates(field).iter().any(|c| {
        api_stems.iter().any(|s| s == c)
            && api_mod.contains(&format!("{c}::register"))
            && docs_lower.contains(c.as_str())
    })
}

/// Namespace name candidates derived from a field: the field itself, plus its
/// singular form (so `particles` matches the `particle` namespace).
fn candidates(field: &str) -> Vec<String> {
    let mut v = vec![field.to_string()];
    if let Some(singular) = field.strip_suffix('s') {
        v.push(singular.to_string());
    }
    v
}

/// Module stems under `src/api/` (the registered namespace module names), minus
/// `mod`: plain `<x>.rs` files AND `<x>/mod.rs` directory modules — a namespace
/// split into a subfolder to fit the size cap (e.g. `animator/`, `nav/`) is still
/// one namespace unit.
pub(super) fn api_stems() -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(API_DIR) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_file_mod = path.extension().is_some_and(|e| e == "rs");
        let is_dir_mod = path.is_dir() && path.join("mod.rs").is_file();
        if is_file_mod || is_dir_mod {
            if let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) {
                if stem != "mod" {
                    out.push(stem);
                }
            }
        }
    }
    out
}

/// Concatenate every `src/editor/` source EXCEPT the Add menu (so an add entry does
/// not, by itself, satisfy the separate inspector-card axis).
pub(super) fn editor_blob() -> String {
    let mut files = Vec::new();
    walk(Path::new(EDITOR_DIR), &mut files);
    let add = normalize(Path::new(ADD_MENU));
    files
        .iter()
        .filter(|p| normalize(p) != add)
        .map(read)
        .collect::<Vec<_>>()
        .join("\n")
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

pub(super) fn read<P: AsRef<Path>>(path: P) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// Every `.md` file directly in `dir`, concatenated — the scripting-API reference is
/// one file per namespace under `docs/api/` (#569).
pub(super) fn read_md_dir(dir: &str) -> String {
    let Ok(entries) = fs::read_dir(dir) else {
        return String::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .map(read)
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("./")
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_singularize_plural_fields() {
        assert_eq!(candidates("particles"), vec!["particles", "particle"]);
        assert_eq!(candidates("camera"), vec!["camera"]);
    }

    #[test]
    fn add_menu_axis_needs_a_standalone_guard() {
        assert!(has_add_menu("particles", "if !world.has_particles(id) {"));
        assert!(!has_add_menu(
            "animator",
            "world.set_animator(id, Some(x));"
        ));
    }
}
