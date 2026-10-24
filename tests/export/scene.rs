//! A saved scene that names an `.fbx`'s mesh (`::Quad`) and extracted texture
//! (`#tex/Red`), loaded from an export, which has the cache but not the `.fbx`.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use super::fbx::{QUAD, SOURCE, START};
use super::{entity, export, play_scenario, project};

/// Where the scene is saved, in the project and so in its export.
const SCENE: &str = "assets/scenes/quad.scene";

/// The game script that loads [`SCENE`].
const LOADER: &str = "assets/scripts/load.lua";

#[test]
fn a_saved_scene_loads_its_model_and_texture_from_an_export() {
    let game = project(&[QUAD], START);
    save_scene(&game);
    // A game script loads the scene, as a main menu would.
    let lua = format!("local L = {{}}\nfunction L.Start(id) Scene.Load({SCENE:?}) end\nreturn L\n");
    std::fs::write(game.join(rusty::core::paths::native(LOADER)), lua).unwrap();
    let text = std::fs::read_to_string(game.join(rusty::core::paths::native(SCENE)))
        .expect("the saved scene");
    let (mesh, texture) = (format!("{SOURCE}::Quad"), format!("{SOURCE}#tex/Red"));
    assert!(text.contains(&mesh) && text.contains(&texture), "{text}");

    let out = crate::temp::dir().join("out");
    assert!(export(&game, &out).status.success());

    let load = format!(
        "Harness.Expect(Harness.AttachPlayerBot({LOADER:?}), \"attached\")\nHarness.Step(3)\n"
    );
    let (ran, results) = play_scenario(&out, &load, &[]);
    assert!(
        ran.status.success() && results["passed"] == true,
        "the scenario passes"
    );
    let quad = entity(&results, "ExportedQuad");
    assert!(quad["bounds"].is_object(), "the mesh loads: {quad}");
    assert_eq!(quad["material"]["texture"], texture.as_str());
    // The fixture's `.dds` is skipped by design; nothing else may be warned of.
    let stderr = String::from_utf8_lossy(&ran.stderr);
    let warned = stderr
        .lines()
        .filter(|l| l.contains("WARN") && !l.contains("skipped FBX texture `Bumps`"));
    assert_eq!(warned.collect::<Vec<_>>(), Vec::<&str>::new());
}

/// Instantiate the quad in an edit session of `game` and save the scene, as an
/// agent driving the editor would.
fn save_scene(game: &Path) {
    let mut session = Command::new(env!("CARGO_BIN_EXE_session"))
        .arg("--project")
        .arg(game)
        .arg("--empty")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the session runs");
    let line = format!("{START}; return Scene.Save({SCENE:?})\n");
    let mut stdin = session.stdin.take().unwrap();
    stdin.write_all(line.as_bytes()).unwrap();
    drop(stdin);
    let out = session.wait_with_output().unwrap();
    let reply = String::from_utf8_lossy(&out.stdout);
    assert!(reply.contains("\"ok\":true"), "the scene is saved: {reply}");
}
