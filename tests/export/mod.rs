//! `make export` end to end (#1267): the `export` binary on a scratch project, then
//! the `play` binary on what it wrote. Both run as children, since opening a project
//! moves the working directory (`core::project::open`), which every test of this
//! binary shares.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

mod audio;
mod blend;
mod fbx;
mod scene;
mod shaders;
mod textures;

/// The import cache, relative to a project (`asset::convert::IMPORT_CACHE_DIR`).
const CACHE: &str = "assets/generated/imported";

pub(super) fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("models")
        .join(name)
}

/// `<test>/game`, a project holding each fixture model copied to
/// `assets/models/<name>` and the game script [`SPAWN`] whose `Start` runs `start`.
/// Never opened, so it has no `project.rusty` until the export opens it.
fn project(models: &[&str], start: &str) -> PathBuf {
    let root = crate::temp::dir().join("game");
    let models_dir = root.join("assets").join("models");
    std::fs::create_dir_all(&models_dir).unwrap();
    for name in models {
        std::fs::copy(fixture(name), models_dir.join(name)).unwrap();
    }
    let script = root.join(rusty::core::paths::native(SPAWN));
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    let lua = format!("local Spawn = {{}}\nfunction Spawn.Start(id)\n{start}\nend\nreturn Spawn\n");
    std::fs::write(script, lua).unwrap();
    root
}

/// The game script the scenario hands the player: it instantiates the models
/// from inside Play, with the whole Lua surface.
const SPAWN: &str = "assets/scripts/spawn.lua";

/// The scenario: the player runs [`SPAWN`], a few frames pass.
pub(super) const SCENARIO: &str = r#"
Harness.Expect(Harness.AttachPlayerBot("assets/scripts/spawn.lua"), "the player runs the script")
Harness.Step(3)
"#;

fn run(mut command: Command) -> Output {
    let out = command.output().expect("the binary runs");
    eprintln!("{}", String::from_utf8_lossy(&out.stdout));
    eprintln!("{}", String::from_utf8_lossy(&out.stderr));
    out
}

/// `export --project <game> --out <out>`.
fn export(game: &Path, out: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_export"));
    command.arg("--project").arg(game).arg("--out").arg(out);
    run(command)
}

/// `play --project <game>` on [`SCENARIO`] with `env` set, and its
/// `results.json`.
fn play(game: &Path, env: &[(&str, &Path)]) -> (Output, Value) {
    play_scenario(game, SCENARIO, env)
}

/// `play --project <game>` on the Lua `scenario` with `env` set, and its
/// `results.json`; the run's folder is `<test>/run-<n>`, outside the project.
fn play_scenario(game: &Path, scenario: &str, env: &[(&str, &Path)]) -> (Output, Value) {
    let tmp = crate::temp::dir();
    let dir = (0..)
        .map(|n| tmp.join(format!("run-{n}")))
        .find(|d| !d.exists())
        .unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let lua = scenario;
    let scenario = dir.join("scenario.lua");
    std::fs::write(&scenario, lua).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_play"));
    command.arg("--project").arg(game).arg(&scenario).arg(&dir);
    command.envs(env.iter().copied());
    let out = run(command);
    let results = std::fs::read_to_string(dir.join("results.json")).expect("results.json");
    (out, serde_json::from_str(&results).unwrap())
}

/// The entity named `name` in a run's final snapshot.
fn entity<'a>(results: &'a Value, name: &str) -> &'a Value {
    let entities = results["final_snapshot"]["entities"].as_array().unwrap();
    let named = entities.iter().find(|e| e["name"] == name);
    named.unwrap_or_else(|| panic!("no {name} in the run's final snapshot"))
}

/// Every file under `dir`, by its `/`-separated relative path, with its bytes.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    collect(dir, dir, &mut files);
    files
}

fn collect(root: &Path, dir: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, files);
        } else {
            let rel = path.strip_prefix(root).unwrap().to_string_lossy();
            files.insert(rel.replace('\\', "/"), std::fs::read(&path).unwrap());
        }
    }
}

/// The files of `tree` with one of `extensions`.
fn with_extension<'a>(tree: &'a BTreeMap<String, Vec<u8>>, extensions: &[&str]) -> Vec<&'a str> {
    let named = |p: &&String| extensions.iter().any(|e| p.ends_with(&format!(".{e}")));
    tree.keys().filter(named).map(String::as_str).collect()
}

/// The files of `tree` under the import cache.
fn cache(tree: &BTreeMap<String, Vec<u8>>) -> BTreeMap<&str, &[u8]> {
    let cached = |(p, _): &(&String, &Vec<u8>)| p.starts_with(CACHE);
    tree.iter()
        .filter(cached)
        .map(|(p, b)| (p.as_str(), b.as_slice()))
        .collect()
}
