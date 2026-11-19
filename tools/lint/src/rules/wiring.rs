//! Every row of [`super::RULES`] is wired: its entry point, run over a fixture it
//! must flag, reports that fixture's planted file. A rule's own tests hold its
//! judgement on a line; this holds that the row the gate runs still reaches it, so a
//! `run` emptied to `vec![]`, or reading somewhere other than the scan, fails here.
//!
//! The cases are keyed by the row's name, so a new row with no case fails the test
//! naming the rule.

use super::scan::Scan;
use super::RULES;
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
pub(super) fn case_for(rule: &str) -> Option<Case> {
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
        "fold-names" => case(
            &[("Cargo.toml", "[package]\nname = \"rusty\"\n"), ("tests/a.rs", "use rusty;\n")],
            "tests/a.rs",
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
        "gpu-default-only" => one(
            "crates/render/src/a.rs",
            "#[cfg(not(feature = \"dev\"))]\nfn f() {}\n",
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
        "test-static-write" => one(
            "crates/app/src/a_tests.rs",
            "static ON: AtomicBool = AtomicBool::new(false);\n#[test]\nfn t() {\n    ON.store(true, Relaxed);\n}\n",
        ),
        "test-temp-dir" => one(
            "tests/a.rs",
            "#[test]\nfn t() {\n    let p = std::env::temp_dir().join(\"x\");\n}\n",
        ),
        "tool-named-test" => one("tests/nav/survivors.rs", "#[test]\nfn t() {}\n"),
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
