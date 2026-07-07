//! Component-completeness gate (#81).
//!
//! Every first-class component must satisfy four axes that deliberately live in layers
//! which don't depend on each other:
//!   1. a field on `Entity` (`src/components/entity.rs`) — the discovery source,
//!   2. an Add Component entry (`src/editor/inspector/components/add.rs`),
//!      guarding on absence through the #344 accessor facade,
//!   3. an inspector card (some `src/editor/inspector/components/*.rs`) writing
//!      through the facade's `_mut`/`set_` accessors,
//!   4. an API namespace (`src/api/<x>.rs` + registration in `src/api/mod.rs`, and a
//!      mention in `docs/scripting-api.md`).
//!
//! Components are DISCOVERED from `Entity`'s `Option<…Component>` fields, so a new
//! component can't dodge the gate. Axis 1 is the discovery source (always present);
//! the other three are checked here by scanning the source. Today's incomplete
//! components are grandfathered in `tools/lint/components_baseline.txt` — a burn-down
//! list of `component axis` lines (#82) — with the particle system deliberately
//! excluded so it is the gate's first fully-green component.
//!
//! Std-only, like the size gate and the determinism guard. The scan is coarse
//! (substring matches on source), matching the existing lint philosophy.
//!
//! ## Waivers (#82)
//! A few axes are *intentionally* unmet because closing them mechanically would
//! fragment the engine's one stable API surface
//! (`Transform`/`Input`/`Time`/`Physics`/`Scene`/`Animator`/`Nav`/
//! `Camera`/`Material`/…). Those live in [`WAIVERS`] — an auditable, in-code list
//! of `(component, axis, rationale)` rows. A waived axis counts as satisfied, but
//! unlike the burn-down baseline each waiver carries its written justification
//! right here in the gate, so the decision is reviewable in `git` and can never
//! be a silent skip. The baseline file is the burn-down list for axes we still
//! intend to close; [`WAIVERS`] is for axes deliberately served by a shared
//! namespace that we will not re-implement standalone.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --components`

use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

const ENTITY: &str = "src/components/entity.rs";
const ADD_MENU: &str = "src/editor/inspector/components/add.rs";
const EDITOR_DIR: &str = "src/editor";
const API_DIR: &str = "src/api";
const API_MOD: &str = "src/api/mod.rs";
const DOCS: &str = "docs/scripting-api.md";
const BASELINE: &str = "tools/lint/components_baseline.txt";
const REPORT: &str = ".lint/report.txt";

/// The checkable axes (axis 1, the `Entity` field, is the discovery source).
const AXES: &[&str] = &["add_menu", "inspector", "api"];

/// Deliberately-waived axes: `(component_field, axis, rationale)`. A waived axis
/// is treated as satisfied. Each row is a documented decision (#82) NOT to add a
/// per-component artifact, because the axis is already served another way and
/// doing so standalone would fragment the one stable API surface or duplicate a
/// content-driven workflow. Reviewable here, never a silent skip.
const WAIVERS: &[(&str, &str, &str)] = &[
    (
        "mesh",
        "add_menu",
        "Mesh is assigned by dragging an asset from the content grid / the render \
         inspector, not picked from the Add Component menu — a blank mesh slot is \
         meaningless. Authoring stays content-driven (CLAUDE.md: glTF/OBJ sources).",
    ),
    (
        "mesh",
        "api",
        "Mesh geometry is content (glTF/OBJ), not a scriptable scalar surface; \
         swapping meshes at runtime is out of the script API's scope. Material \
         look is driven via the `Material` namespace instead.",
    ),
    (
        "material",
        "api",
        "Served by the `Material` namespace (SetTexture/SetMetallic/SetRoughness \
         and their map variants) — the `material` field is a reference to a shared \
         library material, and a dedicated `Material`-field namespace would just \
         restate the same surface.",
    ),
    (
        "collider",
        "api",
        "Served by the `Physics` namespace: the collider is queried via \
         Physics.Raycast plus the #311 spatial surface (Overlap*/Check*, \
         SphereCast, ClosestPoint/ContainsPoint, GetBounds) — the same rapier \
         world the engine casts against. A separate `Collider` namespace would \
         split physics across two surfaces.",
    ),
    (
        "rigidbody",
        "api",
        "Served by the `Physics` namespace \
         (GetVelocity/SetVelocity/AddForce/SetKinematic act on the rigidbody).",
    ),
    (
        "nav_agent",
        "api",
        "Served by the `NavMeshAgent`/`Navigation` namespace in src/api/nav.rs \
         (the field is `nav_agent`, the namespace is the Unity name `NavMeshAgent`).",
    ),
    (
        "visual_correction",
        "api",
        "Served by the `Graphics` namespace, which drives the active \
         VisualCorrectionComponent's bloom/SSR/tonemap/exposure knobs (render-only \
         state). A per-component namespace would duplicate that surface.",
    ),
];

/// Entry point: discover every component and fail on any unbaselined missing axis.
pub fn run() {
    let components = discover();
    let add_src = read(ADD_MENU);
    let editor_blob = editor_blob();
    let api_stems = api_stems();
    let api_mod = read(API_MOD);
    let docs = read(DOCS).to_lowercase();
    let baseline = load_baseline();

    let mut violations = Vec::new();
    for field in &components {
        for axis in AXES {
            let ok = match *axis {
                "add_menu" => has_add_menu(field, &add_src),
                "inspector" => has_inspector(field, &editor_blob),
                "api" => has_api(field, &api_stems, &api_mod, &docs),
                _ => true,
            };
            if !ok && !waived(field, axis) && !baselined(field, axis, &baseline) {
                violations.push(format!(
                    "INCOMPLETE_COMPONENT `{field}` missing `{axis}` axis"
                ));
            }
        }
    }
    report(&violations);
    if violations.is_empty() {
        println!("components: ok");
    } else {
        exit(1);
    }
}

/// Discover components from `Entity`'s `pub <field>: Option<…Component>` lines.
/// Shared with the parity gate (`parity.rs`), so both gates enumerate first-class
/// components from the same non-fragile source — `Entity`'s component fields.
pub(crate) fn discover() -> Vec<String> {
    discover_from(&read(ENTITY))
}

/// Pure core of [`discover`], over the `entity.rs` source text (so it is testable
/// without depending on the process working directory).
fn discover_from(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in src.lines() {
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

/// Axis 2: a standalone Add Component entry guards on absence via the #344
/// accessor facade (`!world.has_<field>(id)`). A component only added as a
/// side-effect of another has no such guard and is reported until it gets its
/// own entry (#82).
fn has_add_menu(field: &str, add_src: &str) -> bool {
    add_src.contains(&format!("!world.has_{field}(id)"))
}

/// Axis 3: some editor file other than the Add menu edits the component in a
/// card — through the facade, that is a mutable accessor (`.<field>_mut(`) or a
/// detach/attach write (`.set_<field>(`).
fn has_inspector(field: &str, editor_blob: &str) -> bool {
    editor_blob.contains(&format!(".{field}_mut("))
        || editor_blob.contains(&format!(".set_{field}("))
}

/// Axis 4: an API namespace named after the component (its field, or the singular
/// of a plural field) exists, is registered in `api/mod.rs`, and is documented.
fn has_api(field: &str, api_stems: &[String], api_mod: &str, docs_lower: &str) -> bool {
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
fn api_stems() -> Vec<String> {
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
fn editor_blob() -> String {
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

fn read<P: AsRef<Path>>(path: P) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("./")
        .replace('\\', "/")
}

/// Baseline entries are `component axis` pairs (whitespace-separated); `#` comments
/// and blank lines are ignored.
fn load_baseline() -> Vec<(String, String)> {
    read(BASELINE)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once(char::is_whitespace))
        .map(|(c, a)| (c.trim().to_string(), a.trim().to_string()))
        .collect()
}

fn baselined(field: &str, axis: &str, baseline: &[(String, String)]) -> bool {
    baseline.iter().any(|(c, a)| c == field && a == axis)
}

/// True when `(field, axis)` is a documented [`WAIVERS`] decision — served by a
/// shared namespace or a content-driven workflow, not to be implemented standalone.
fn waived(field: &str, axis: &str) -> bool {
    WAIVERS.iter().any(|(c, a, _)| *c == field && *a == axis)
}

fn report(violations: &[String]) {
    let mut body = if violations.is_empty() {
        String::from("components: ok\n")
    } else {
        String::from("components: FAILED\n")
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
            "\n{} incomplete component axis(es). Complete the axis, or add a \
             `component axis` line to {BASELINE} (burn-down only — see docs/linting.md).",
            violations.len()
        );
    }
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

    #[test]
    fn discovers_optional_component_fields_only() {
        let src = "\
            pub transform: TransformComponent,\n\
            pub camera: Option<CameraComponent>,\n\
            #[serde(default)]\n\
            pub particles: Option<ParticleEmitterComponent>,\n\
            pub parent_id: Option<u32>,\n";
        let fields = discover_from(src);
        assert!(fields.iter().any(|f| f == "particles"));
        assert!(fields.iter().any(|f| f == "camera"));
        // `transform` is mandatory (not Option) and `parent_id: Option<u32>` is not
        // a component — neither is discovered.
        assert!(!fields.iter().any(|f| f == "parent_id"));
        assert!(!fields.iter().any(|f| f == "transform"));
    }

    #[test]
    fn waivers_are_recognized_and_scoped() {
        // Every waiver row is recognized for its own (component, axis)…
        for (c, a, rationale) in WAIVERS {
            assert!(waived(c, a), "{c}/{a} should be waived");
            assert!(!rationale.is_empty(), "{c}/{a} needs a rationale");
        }
        // …and a waiver does not leak to a different axis of the same component.
        assert!(waived("collider", "api"));
        assert!(!waived("collider", "inspector"));
        // A component with no waiver (e.g. particles) is never waived.
        assert!(!waived("particles", "api"));
    }

    #[test]
    fn light_api_is_not_waived_it_is_implemented() {
        // `light` gets a real `src/api/light.rs` namespace (#82), so it must NOT be
        // on the waiver list — removing the namespace must make the gate fail.
        assert!(!waived("light", "api"));
    }
}
