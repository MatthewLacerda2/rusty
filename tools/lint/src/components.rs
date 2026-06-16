//! Component-completeness gate — every built-in component must satisfy its axes.
//!
//! A component is only "done" when wired across layers that deliberately don't
//! depend on each other (`components/` can't see egui or mlua), so a compile-time
//! enum can't enforce it. This std-only scanner can, like the size and
//! `--determinism` gates: discover components, assert each axis, grandfather gaps in
//! a burn-down baseline. The five axes: **field** (an `Option<…>` on `Entity`),
//! **add_menu** (a dedicated `entity.<field>.is_none(` entry), **inspector**
//! (`fn draw_<field>` or an `inspector_<field>.rs` card), **api** (a registered API
//! namespace serving it), **docs** (that namespace in `docs/scripting-api.md`).
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --components`
//! Exit 1 on any un-baselined gap (or a stale baseline line); `.lint/report.txt`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

const COMPONENTS_DIR: &str = "src/components";
const ENTITY_RS: &str = "src/components/entity.rs";
const ADD_MENU_RS: &str = "src/editor/inspector_add.rs";
const EDITOR_DIR: &str = "src/editor";
const API_DIR: &str = "src/api";
const API_MOD_RS: &str = "src/api/mod.rs";
const DOCS: &str = "docs/scripting-api.md";
const BASELINE: &str = "tools/lint/components_baseline.txt";
const REPORT: &str = ".lint/report.txt";

/// Entry point: scan the component surface and fail on any un-grandfathered gap.
pub fn run() {
    let entity = read(ENTITY_RS);
    let add_menu = read(ADD_MENU_RS);
    let api_mod = read(API_MOD_RS);
    let docs = read(DOCS);
    let editor = read_rs(Path::new(EDITOR_DIR));
    let mut api: Vec<(PathBuf, String)> = read_rs(Path::new(API_DIR));
    api.sort_by(|a, b| a.0.cmp(&b.0)); // deterministic serving-module pick

    let mut violations = Vec::new();
    for st in discover() {
        let Some(field) = find_field(&entity, &st) else {
            violations.push(format!("{} field", snake(&st)));
            continue;
        };
        if !add_menu.contains(&format!("entity.{field}.is_none")) {
            violations.push(format!("{field} add_menu"));
        }
        if !has_inspector(&editor, &field) {
            violations.push(format!("{field} inspector"));
        }
        check_api(&field, &api, &api_mod, &docs, &mut violations);
    }
    violations.sort();
    violations.dedup();

    let baseline = load_baseline();
    let active: Vec<String> = violations
        .iter()
        .filter(|v| !baseline.contains(v))
        .cloned()
        .collect();
    // A baseline line with no matching gap is resolved — drop it (the list only shrinks).
    let stale: Vec<String> = baseline
        .iter()
        .filter(|b| !violations.contains(b))
        .cloned()
        .collect();

    report(&active, &stale);
    if active.is_empty() && stale.is_empty() {
        println!("components: ok");
    } else {
        exit(1);
    }
}

/// Every `…Component` struct under `src/components/`, minus mandatory `Transform`.
fn discover() -> Vec<String> {
    let mut out = Vec::new();
    for (path, src) in read_rs(Path::new(COMPONENTS_DIR)) {
        let name = file_name(&path);
        if name == "entity.rs" || name == "mod.rs" || name == "transform.rs" {
            continue;
        }
        for line in src.lines() {
            if let Some(s) = struct_name(line) {
                out.push(s);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn struct_name(line: &str) -> Option<String> {
    // Pull `Foo` out of `pub struct FooComponent {`; only `…Component` structs count.
    let rest = line.trim().strip_prefix("pub struct ")?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    name.ends_with("Component").then_some(name)
}

/// The `Entity` field whose type is `Option<St>`, e.g. `particles` (None if absent).
fn find_field(entity: &str, st: &str) -> Option<String> {
    let pat = format!("Option<{st}>"); // e.g. `Option<ParticleEmitterComponent>`
    for line in entity.lines() {
        if line.contains(&pat) {
            let l = line.trim().trim_start_matches("pub ");
            let colon = l.find(':')?;
            return Some(l[..colon].trim().to_string());
        }
    }
    None
}

/// Inspector card: some editor file defines `fn draw_<field>`, or a dedicated
/// `inspector_<field>.rs` exists (the particle card's shape).
fn has_inspector(editor: &[(PathBuf, String)], field: &str) -> bool {
    let draw_fn = format!("fn draw_{field}(");
    let card_file = format!("inspector_{field}.rs");
    editor
        .iter()
        .any(|(p, src)| src.contains(&draw_fn) || file_name(p) == card_file)
}

/// API axis (+ its docs sub-axis): find the registered namespace serving this
/// component, then assert it is documented. A module serves the component if it is
/// named after the field (a resource namespace, e.g. `camera.rs`) or reads
/// `entity.<field>`.
fn check_api(
    field: &str,
    api: &[(PathBuf, String)],
    api_mod: &str,
    docs: &str,
    violations: &mut Vec<String>,
) {
    let serving = api.iter().find(|(p, src)| {
        let stem = file_name(p).trim_end_matches(".rs").to_string();
        stem == field || contains_field_access(src, field)
    });
    let Some((path, src)) = serving else {
        violations.push(format!("{field} api"));
        return;
    };
    let stem = file_name(path).trim_end_matches(".rs").to_string();
    let registered = api_mod.contains(&format!("{stem}::register"));
    let namespaces = registered_globals(src);
    if !registered || namespaces.is_empty() {
        violations.push(format!("{field} api"));
    } else if !namespaces.iter().any(|ns| docs.contains(ns)) {
        violations.push(format!("{field} docs"));
    }
}

fn registered_globals(src: &str) -> Vec<String> {
    // Namespace names registered as Lua globals — the `"X"` in each `globals()…set("X"`.
    let (mut out, mut idx) = (Vec::new(), 0);
    while let Some(g) = src[idx..].find("globals()") {
        let after = idx + g + "globals()".len();
        if let Some(s) = src[after..].find(".set(\"") {
            let q = after + s + ".set(\"".len();
            if let Some(e) = src[q..].find('"') {
                out.push(src[q..q + e].to_string());
            }
        }
        idx = after;
    }
    out
}

/// True if `src` reads `.field` as an accessor, boundary-checked so `.health`
/// matches but `.health_bar` does not.
fn contains_field_access(src: &str, field: &str) -> bool {
    let ident = |b: u8| b == b'_' || b.is_ascii_alphanumeric();
    let needle = format!(".{field}");
    let bytes = src.as_bytes();
    let mut i = 0;
    while let Some(p) = src[i..].find(&needle) {
        let end = i + p + needle.len();
        if end >= bytes.len() || !ident(bytes[end]) {
            return true;
        }
        i += p + 1;
    }
    false
}

/// PascalCase → snake_case, for the rare struct-without-an-entity-field report.
fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i != 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn load_baseline() -> Vec<String> {
    fs::read_to_string(BASELINE)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn read_rs(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    walk(dir, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().map_or(false, |e| e == "rs") {
            if let Ok(src) = fs::read_to_string(&path) {
                out.push((path, src));
            }
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn report(active: &[String], stale: &[String]) {
    let ok = active.is_empty() && stale.is_empty();
    let mut body = String::from(if ok {
        "components: ok\n"
    } else {
        "components: FAILED\n"
    });
    body.extend(active.iter().map(|v| format!("INCOMPLETE_COMPONENT {v}\n")));
    body.extend(stale.iter().map(|s| format!("STALE_BASELINE {s}\n")));
    fs::create_dir_all(".lint").ok();
    fs::write(REPORT, &body).ok();
    if !ok {
        eprint!("{body}");
        eprintln!(
            "\n{} axis/axes missing, {} stale. Close the axis or grandfather it; delete \
             satisfied lines. Burn-down lives in {BASELINE} (see docs/linting.md).",
            active.len(),
            stale.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_name_only_matches_component_structs() {
        let h = struct_name("pub struct HealthComponent {");
        assert_eq!(h.as_deref(), Some("HealthComponent"));
        assert_eq!(struct_name("pub struct Particle {"), None);
        assert_eq!(struct_name("    let x = 1;"), None);
    }

    #[test]
    fn find_field_reads_the_entity_option() {
        let entity = "    pub particles: Option<ParticleEmitterComponent>,\n";
        let f = find_field(entity, "ParticleEmitterComponent");
        assert_eq!(f.as_deref(), Some("particles"));
        assert_eq!(find_field(entity, "MeshComponent"), None);
    }

    #[test]
    fn field_access_is_boundary_checked() {
        assert!(contains_field_access("e.health.as_ref()", "health"));
        assert!(!contains_field_access("e.health_bar", "health"));
        assert!(!contains_field_access("e.mesh_data", "mesh"));
    }

    #[test]
    fn registered_globals_extracts_namespace_names() {
        let src = "lua.globals()\n .set(\"Particles\", table)";
        assert_eq!(registered_globals(src), vec!["Particles".to_string()]);
        assert_eq!(snake("NavMeshAgentComponent"), "nav_mesh_agent_component");
    }
}
