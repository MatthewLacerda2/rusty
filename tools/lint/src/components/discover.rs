//! Component discovery: first-class components are the rows of the registry
//! (`src/components/registry.rs`'s `first_class!`, #2023), from which `Entity`'s
//! fields are generated, so a new component can't dodge the gates that enumerate
//! them.
//!
//! The registry is *located* (the file under `src/components/` declaring `macro_rules!
//! first_class`), never read from a fixed path: when `Entity` moved from `entity.rs`
//! to `entity/mod.rs` the old constant read an empty string and both gates checked
//! zero components for weeks (#959). Not finding it, or finding no rows in it,
//! fails loudly.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use super::axes::read;

/// Where the registry lives (searched recursively).
pub(super) const COMPONENTS_DIR: &str = "src/components";
const DECL: &str = "macro_rules! first_class {";
const REPORT: &str = ".lint/report.txt";

/// Discover components from the registry's `<field>: <path> => <Kind> [...];` rows.
/// Shared with the parity gate (`parity.rs`), so both gates enumerate first-class
/// components from the same source. Exits 1 (`NO_REGISTRY` / `NO_COMPONENTS`) rather
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
    let Some(src) = registry_source(&dir) else {
        return Err(format!(
            "NO_REGISTRY no `{DECL}` found under `{COMPONENTS_DIR}/` — the gates \
             discover components from it; point `discover.rs` at where it moved"
        ));
    };
    let fields = discover_from(&src);
    if fields.is_empty() {
        return Err(format!(
            "NO_COMPONENTS `{DECL}` has no `<field>: <path> => <Kind> [...];` rows — \
             the discovery pattern no longer matches how the registry is written"
        ));
    }
    Ok(fields)
}

/// The text of the first `.rs` file under `dir` declaring the registry.
pub(super) fn registry_source(dir: &Path) -> Option<String> {
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

/// Pure core of [`discover`]: the `(field, Kind)` of each registry row in `src`.
pub(super) fn rows(src: &str) -> Vec<(String, String)> {
    let body = src.split_once(DECL).map_or("", |(_, rest)| rest);
    body.lines()
        .filter_map(|line| {
            let (field, rest) = line.trim().split_once(": ")?;
            let (_, rest) = rest.split_once(" => ")?;
            let (kind, accessors) = rest.split_once(" [")?;
            let ident =
                |s: &str| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_');
            (ident(field) && ident(kind) && accessors.ends_with("];"))
                .then(|| (field.to_string(), kind.to_string()))
        })
        .collect()
}

/// The component fields of the registry in `src`.
fn discover_from(src: &str) -> Vec<String> {
    rows(src).into_iter().map(|(field, _)| field).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_registry_rows_only() {
        let src = "macro_rules! first_class {\n\
            ($cb:ident) => {\n\
            // the rows\n\
            camera: rendering::camera::CameraComponent => Camera [camera_mut has_camera];\n\
            particles: particle::ParticleEmitterComponent => Particles [particles_mut];\n\
            };\n\
            (@rows $callback:ident $($field:ident: $($seg:ident)::+ => $kind:ident [$($acc:ident)*];)*) => {\n\
            }\n";
        let fields = discover_from(src);
        assert_eq!(fields, vec!["camera", "particles"]);
        assert_eq!(rows(src)[1], ("particles".into(), "Particles".into()));
    }

    #[test]
    fn real_tree_is_discovered() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fields = discover_in(&root).expect("the registry must be found in the real tree");
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
        assert!(err.starts_with("NO_REGISTRY"), "{err}");
        fs::write(
            dir.join("e.rs"),
            "macro_rules! first_class {\n    () => {};\n}\n",
        )
        .unwrap();
        let err = discover_in(&tmp).unwrap_err();
        assert!(err.starts_with("NO_COMPONENTS"), "{err}");
        fs::remove_dir_all(&tmp).ok();
    }
}
