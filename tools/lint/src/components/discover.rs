//! Component discovery: first-class components are `Entity`'s `Option<…Component>`
//! fields, so a new component can't dodge the gates that enumerate them.
//!
//! `Entity` is *located* (the file under `src/components/` declaring `pub struct
//! Entity`), never read from a fixed path: when it moved from `entity.rs` to
//! `entity/mod.rs` the old constant read an empty string and both gates checked
//! zero components for weeks (#959). Not finding it, or finding no component
//! fields in it, fails loudly.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use super::axes::read;

/// Where `Entity` lives (searched recursively).
const COMPONENTS_DIR: &str = "src/components";
const DECL: &str = "pub struct Entity {";
const REPORT: &str = ".lint/report.txt";

/// Discover components from `Entity`'s `pub <field>: Option<…Component>` lines.
/// Shared with the parity gate (`parity.rs`), so both gates enumerate first-class
/// components from the same source. Exits 1 (`NO_ENTITY` / `NO_COMPONENTS`) rather
/// than ever returning an empty list.
pub(crate) fn discover() -> Vec<String> {
    discover_in(Path::new(".")).unwrap_or_else(|e| {
        let body = format!("components: FAILED\n{e}\n");
        fs::create_dir_all(".lint").ok();
        fs::write(REPORT, &body).ok();
        eprint!("{body}");
        exit(1);
    })
}

/// [`discover`] under the repo `root`, as a result (testable from any directory).
pub(crate) fn discover_in(root: &Path) -> Result<Vec<String>, String> {
    let dir = root.join(COMPONENTS_DIR);
    let Some(src) = entity_source(&dir) else {
        return Err(format!(
            "NO_ENTITY no `{DECL}` found under `{COMPONENTS_DIR}/` — the gates \
             discover components from it; point `discover.rs` at where it moved"
        ));
    };
    let fields = discover_from(&src);
    if fields.is_empty() {
        return Err(format!(
            "NO_COMPONENTS `{DECL}` has no `pub <field>: Option<…Component>` fields — \
             the discovery pattern no longer matches how `Entity` is written"
        ));
    }
    Ok(fields)
}

/// The text of the first `.rs` file under `dir` declaring `Entity`.
fn entity_source(dir: &Path) -> Option<String> {
    let mut files = Vec::new();
    walk(dir, &mut files);
    files.sort();
    files.iter().map(read).find(|s| s.contains(DECL))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Pure core of [`discover`]: the component fields of the `Entity` struct body in
/// `src` (fields of any other struct in the same file are not components).
fn discover_from(src: &str) -> Vec<String> {
    let body = src
        .split_once(DECL)
        .map_or("", |(_, rest)| rest.split("\n}").next().unwrap_or(""));
    let mut out = Vec::new();
    for line in body.lines() {
        let Some(rest) = line.trim().strip_prefix("pub ") else {
            continue;
        };
        let Some((field, ty)) = rest.split_once(": Option<") else {
            continue;
        };
        // `Option<…Component>,` — only optional component fields, never `parent_id`.
        let inner = ty.trim_end_matches([',', ' ']).trim_end_matches('>');
        if inner.ends_with("Component") {
            out.push(field.trim().to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_optional_component_fields_only() {
        let src = "pub struct Entity {\n\
            pub transform: TransformComponent,\n\
            pub camera: Option<CameraComponent>,\n\
            #[serde(default)]\n\
            pub particles: Option<ParticleEmitterComponent>,\n\
            pub parent_id: Option<u32>,\n\
            }\n\
            pub struct Other {\n\
            pub stray: Option<StrayComponent>,\n\
            }\n";
        let fields = discover_from(src);
        assert_eq!(fields, vec!["camera", "particles"]);
    }

    #[test]
    fn real_tree_is_discovered() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fields = discover_in(&root).expect("Entity must be found in the real tree");
        for f in ["material", "camera", "particles"] {
            assert!(fields.iter().any(|x| x == f), "`{f}` not discovered");
        }
    }

    #[test]
    fn missing_entity_or_fields_fail_loudly() {
        let tmp = std::env::temp_dir().join(format!("lint-discover-{}", std::process::id()));
        let dir = tmp.join(COMPONENTS_DIR);
        fs::create_dir_all(&dir).unwrap();
        let err = discover_in(&tmp).unwrap_err();
        assert!(err.starts_with("NO_ENTITY"), "{err}");
        fs::write(
            dir.join("e.rs"),
            "pub struct Entity {\n    pub id: u32,\n}\n",
        )
        .unwrap();
        let err = discover_in(&tmp).unwrap_err();
        assert!(err.starts_with("NO_COMPONENTS"), "{err}");
        fs::remove_dir_all(&tmp).ok();
    }
}
