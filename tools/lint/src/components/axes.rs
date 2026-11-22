use std::fs;
use std::path::{Path, PathBuf};

use super::ADD_MENU;

const EDITOR_DIR: &str = "src/editor";
const API_DIR: &str = "src/script/api";

pub(super) fn has_add_menu(field: &str, add_src: &str) -> bool {
    add_src.contains(&format!(".has_{field}(id)"))
}

pub(super) fn has_inspector(field: &str, editor_blob: &str) -> bool {
    editor_blob.contains(&format!(".{field}_mut("))
        || editor_blob.contains(&format!(".set_{field}("))
}

pub(super) fn has_api(field: &str, api_stems: &[String], api_mod: &str, docs_lower: &str) -> bool {
    candidates(field).iter().any(|c| {
        api_stems.iter().any(|s| s == c)
            && api_mod.contains(&format!("{c}::register"))
            && (docs_lower.contains(c.as_str()) || docs_lower.contains(&c.replace('_', "")))
    })
}

fn candidates(field: &str) -> Vec<String> {
    let mut v = vec![field.to_string()];
    if let Some(singular) = field.strip_suffix('s') {
        v.push(singular.to_string());
    }
    v
}

pub(super) fn api_stems(base: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(crate::modules::real(base, API_DIR)) else {
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

pub(super) fn add_menu_src(base: &Path) -> String {
    let mut files = Vec::new();
    walk(&crate::modules::real(base, ADD_MENU), &mut files);
    files.iter().map(read).collect::<Vec<_>>().join("\n")
}

pub(super) fn editor_blob(base: &Path) -> String {
    let mut files = Vec::new();
    walk(&crate::modules::real(base, EDITOR_DIR), &mut files);
    let add = format!("{}/", normalize(&crate::modules::real(base, ADD_MENU)));
    files
        .iter()
        .filter(|p| !normalize(p).starts_with(&add))
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

pub(super) fn read_md_dir(dir: &Path) -> String {
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
    path.to_string_lossy().trim_start_matches("./").to_string()
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
        assert!(has_add_menu("mask", "|w, id| w.has_mask(id),"));
        assert!(!has_add_menu(
            "animator",
            "world.set_animator(id, Some(x));"
        ));
    }

    #[test]
    fn api_axis_accepts_the_lua_spelling_in_docs() {
        let stems = vec!["layout_group".to_string()];
        let reg = "layout_group::register(lua, scope, scene)?;";
        assert!(has_api("layout_group", &stems, reg, "# layoutgroup"));
        assert!(!has_api("layout_group", &stems, reg, "# nothing here"));
        assert!(!has_api("layout_group", &stems, "", "# layoutgroup"));
    }
}
