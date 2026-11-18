//! Every row of [`super::RULES`] is wired: its entry point, run over a fixture it
//! must flag, reports that fixture's planted file. A rule's own tests hold its
//! judgement on a line; this holds that the row the gate runs still reaches it, so a
//! `run` emptied to `vec![]`, or reading somewhere other than the scan, fails here.
//!
//! The cases are keyed by the row's name, so a new row with no case fails the test
//! naming the rule.

use super::scan::Scan;
use super::{Allow, RULES};
use crate::repo::scaffold::Repo;

/// A rule's must-flag case: the files of a scratch repository, and the one file
/// the rule must report.
pub struct Case {
    pub files: Vec<(String, String)>,
    pub flags: &'static str,
}

fn case(files: &[(&str, &str)], flags: &'static str) -> Case {
    Case {
        files: files
            .iter()
            .map(|(p, b)| (p.to_string(), b.to_string()))
            .collect(),
        flags,
    }
}

/// One planted file, the one the rule must report.
fn one(path: &'static str, body: &str) -> Case {
    case(&[(path, body)], path)
}

/// The must-flag case of the rule named `rule`, `None` when it has none.
fn case_for(rule: &str) -> Option<Case> {
    let app = "crates/app/src/a.rs";
    Some(match rule {
        "active-self" => one(app, "fn f(w: &W) -> bool {\n    w.is_active_self(e)\n}\n"),
        "api-parity" => case(
            &[
                (
                    "crates/editor/src/a.rs",
                    "fn f() {\n    rusty_edit::authoring::light::set_range(c, 1.0);\n}\n",
                ),
                (
                    "crates/edit/src/authoring/light.rs",
                    "pub fn set_range(c: &mut C, v: f32) {\n    c.range = v;\n}\n",
                ),
            ],
            "crates/edit/src/authoring/light.rs",
        ),
        "asset-write" => one(app, "fn f() {\n    std::fs::write(p, b);\n}\n"),
        "atomic-write" => one(app, "fn f() {\n    std::fs::rename(a, b);\n}\n"),
        "binary-init" => one("src/main.rs", "fn main() {\n    rusty::logging::init();\n}\n"),
        "body-table" => one(
            "crates/physics/src/a.rs",
            "fn f(b: &mut B) {\n    b.set_body_type(t, true);\n}\n",
        ),
        "cfg-test-switch" => one(app, "fn f() -> bool {\n    cfg!(test)\n}\n"),
        "converter-log" => one(
            "crates/asset/src/convert/x.rs",
            "fn f() {\n    log::warn!(\"[Asset] x\");\n}\n",
        ),
        "dir-size" => {
            let mut files = vec![("crates/app/src/many/mod.rs".to_string(), String::new())];
            files.extend((0..13).map(|i| (format!("crates/app/src/many/f{i}.rs"), String::new())));
            Case {
                files,
                flags: "crates/app/src/many/mod.rs",
            }
        }
        "drag-range" => one(
            "crates/editor/src/a.rs",
            "fn f(ui: &mut Ui) {\n    ui.add(egui::Slider::new(&mut v, 0.0..=1.0));\n}\n",
        ),
        "ecs-write" => one(
            "crates/ecs/src/a.rs",
            "fn f(w: &mut W) {\n    w.inner.spawn(());\n}\n",
        ),
        "engine-spawn" => one(
            "tests/a.rs",
            "const B: &str = env!(\"CARGO_BIN_EXE_session\");\n",
        ),
        "entity-arg" => one(
            "crates/script/src/api/a.rs",
            "fn f(lua: &Lua) {\n    lua.create_function(|_, id: u32| Ok(()));\n}\n",
        ),
        "full-literal" => case(
            &[
                (
                    "crates/components/src/entity/mod.rs",
                    "pub struct Entity {\n    pub id: u32,\n}\n",
                ),
                (
                    "crates/components/src/registry.rs",
                    "macro_rules! first_class {\n    collider: physics::ColliderComponent => Collider [];\n}\n",
                ),
                (
                    "crates/components/src/collider/mod.rs",
                    "pub struct ColliderComponent {\n    pub a: u8,\n    pub b: u8,\n    pub c: u8,\n}\n",
                ),
                (
                    "tests/a.rs",
                    "#[test]\nfn t() {\n    let c = ColliderComponent { a: 1, b: 2, c: 3 };\n}\n",
                ),
            ],
            "tests/a.rs",
        ),
        "fullscreen-triangle" => one(
            "engine/shaders/a.wgsl",
            "fn vs(i: u32) -> f32 {\n    let x = f32((i << 1u) & 2u);\n    return x;\n}\n",
        ),
        "gpu-ops" => one(
            "crates/render/src/passes/a.rs",
            "fn f(q: &wgpu::Queue) {\n    q.submit([]);\n}\n",
        ),
        "ignored" => case(
            &[(".gitignore", "secret.rs\n"), ("crates/app/src/secret.rs", "")],
            "crates/app/src/secret.rs",
        ),
        "key-guard" => case(
            &[
                (
                    "Cargo.toml",
                    "[package]\nname = \"t\"\n\n[dev-dependencies]\nrusty-dev = { path = \"d\" }\n",
                ),
                ("src/lib.rs", ""),
            ],
            "Cargo.toml",
        ),
        "log-tag" => one(app, "fn f() {\n    log::warn!(\"no tag\");\n}\n"),
        "markers" => one(app, concat!("// TO", "DO: later\n")),
        "named-enum" => one(
            app,
            "impl T {\n    fn parse(s: &str) -> Option<Self> {\n        None\n    }\n    \
             fn name(self) -> &'static str {\n        \"\"\n    }\n}\n",
        ),
        "orphan-module" => case(
            &[
                ("Cargo.toml", "[package]\nname = \"t\"\nversion = \"0.1.0\"\n"),
                ("src/lib.rs", ""),
                ("src/lost.rs", ""),
            ],
            "src/lost.rs",
        ),
        "orphan-seeds" => case(
            &[("src/lib.rs", ""), ("proptest-regressions/gone.txt", "cc 0\n")],
            "proptest-regressions/gone.txt",
        ),
        "physics-borrow" => one(
            "crates/physics/src/a.rs",
            "fn f(s: &mut S) {\n    s.bodies.get_mut(h);\n}\n",
        ),
        "platform-cfg" => one(app, "#[cfg(windows)]\nfn f() {}\n"),
        "prefix-group" => case(
            &[
                ("crates/app/src/x/mod.rs", "mod nav_a;\nmod nav_b;\nmod nav_c;\n"),
                ("crates/app/src/x/nav_a.rs", ""),
                ("crates/app/src/x/nav_b.rs", ""),
                ("crates/app/src/x/nav_c.rs", ""),
            ],
            "crates/app/src/x/mod.rs",
        ),
        "read-keys" => one(app, "fn f() {\n    dev::providers::read_keys();\n}\n"),
        "root-tests" => case(
            &[
                (
                    "Cargo.toml",
                    "[package]\nname = \"t\"\nautobins = false\n\n[[bin]]\nname = \"b\"\n",
                ),
                ("src/main.rs", ""),
            ],
            "Cargo.toml",
        ),
        "sanitize" => one(
            "crates/edit/src/authoring/a.rs",
            "pub fn set_x(c: &mut C, v: f32) {\n    c.x = v;\n}\n",
        ),
        "self-path" => one(app, "//! crates/app/src/b.rs — not this file\n"),
        "skip" => one("tests/a.rs", "#[test]\n#[ignore]\nfn t() {}\n"),
        "sorted" => one(
            app,
            "// sorted: order is free\nmod b;\nmod a;\n",
        ),
        "stored-format" => one(
            "crates/x/src/f.rs",
            "fn save(m: &M) {\n    let t = serde_json::to_vec(m).unwrap();\n    written::write(p, &t).unwrap();\n}\n",
        ),
        "subprocess" => one(app, "fn f() {\n    std::process::Command::new(\"x\");\n}\n"),
        "test-inner-cfg" => one("tests/a.rs", "#![cfg(feature = \"dev\")]\n#[test]\nfn t() {}\n"),
        "test-temp-dir" => one(
            "tests/a.rs",
            "#[test]\nfn t() {\n    let p = std::env::temp_dir().join(\"x\");\n}\n",
        ),
        "unwrap-deny" => one("crates/physics/src/lib.rs", "//! A sim crate.\n"),
        "world-space" => one(app, "fn f(m: Mat4) {\n    m.to_scale_rotation_translation();\n}\n"),
        _ => return None,
    })
}

#[test]
fn every_rule_reports_its_must_flag_case_through_its_row() {
    let mut broken = Vec::new();
    for (name, rule, _) in RULES {
        let Some(case) = case_for(name) else {
            broken.push(format!("`{name}` has no must-flag case in `case_for`"));
            continue;
        };
        let repo = Repo::new(&format!("wiring-{name}"), true);
        for (path, body) in &case.files {
            repo.file(path, body);
        }
        repo.git(&["add", "-A"]);
        let found = rule(&Scan::at(&repo.0));
        if !found
            .iter()
            .any(|f| f.path.ends_with(case.flags) && f.rule == *name)
        {
            let got: Vec<String> = found.iter().map(ToString::to_string).collect();
            broken.push(format!("`{name}` did not report `{}`: {got:?}", case.flags));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

/// The findings `all` keeps of rule `name` on `path`, over `repo`.
fn kept(repo: &Repo, name: &str, path: &str) -> usize {
    let all = super::all(&Scan::at(&repo.0));
    all.iter()
        .filter(|f| f.path == path && f.rule == name)
        .count()
}

/// The rules nothing may excuse, said here apart from `RULES`' `Allow` column so
/// that switching a row's mode disagrees with it.
const UNEXCUSABLE: &[&str] = &["ignored", "key-guard", "orphan-seeds", "root-tests"];

/// Every row's `Allow` is the one meant: an inline allow above its must-flag
/// finding excuses it (through the shared filter, or as an `Own` rule reads it)
/// unless the rule is [`UNEXCUSABLE`], and only a `Never` row's survives it. A
/// manifest's or a seed file's finding is one the shared filter cannot read, so
/// `Inline` on `key-guard` fails the second half.
#[test]
fn an_inline_allow_on_each_row_s_finding_is_honoured_by_its_mode() {
    let mut broken = Vec::new();
    for (name, rule, allow) in RULES {
        let Some(case) = case_for(name) else { continue };
        let repo = Repo::new(&format!("wiring-allow-{name}"), true);
        for (path, body) in &case.files {
            repo.file(path, body);
        }
        repo.git(&["add", "-A"]);
        let Some(hit) = rule(&Scan::at(&repo.0))
            .into_iter()
            .find(|f| f.path.ends_with(case.flags) && f.rule == *name)
        else {
            continue; // the test above names it
        };
        let before = kept(&repo, name, &hit.path);
        let text = std::fs::read_to_string(repo.0.join(&hit.path)).unwrap_or_default();
        let mut lines: Vec<String> = text.lines().map(String::from).collect();
        let at = (hit.line.max(1) - 1).min(lines.len());
        lines.insert(at, format!("// lint: allow({name}) wiring"));
        repo.file(&hit.path, &(lines.join("\n") + "\n"));
        let after = kept(&repo, name, &hit.path);
        let excused = after < before;
        if excused == UNEXCUSABLE.contains(name) || excused == (*allow == Allow::Never) {
            broken.push(format!(
                "`{name}`: an inline allow above it: {before} kept, then {after}"
            ));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

/// An `Own` row skips the shared filter, so what the rule reports about its own
/// allows survives one: `api-parity`'s stale allow, on an op no inspector writes.
#[test]
fn an_own_row_s_report_on_its_own_allow_is_not_excused_by_it() {
    let repo = Repo::new("wiring-own", true);
    let op = "crates/edit/src/authoring/light.rs";
    repo.file(
        op,
        "pub fn set_range(c: &mut C) {} // lint: allow(api-parity) why\n",
    );
    repo.git(&["add", "-A"]);
    assert_eq!(kept(&repo, "api-parity", op), 1);
}
