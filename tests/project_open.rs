//! Every entry point opens a project the same way (#1652): the headless session and
//! `play` answer `project.rusty`'s startup scene and quality preset, not the
//! engine's defaults, and the session writes a setter back into the file as the
//! editor does. The binaries run as children, since opening a project moves the
//! working directory. (The editor's and the player's worlds are tested in-crate.)

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SCENE: &str = "assets/scenes/arena.scene";

/// `play_project`'s game, its startup scene `arena.scene`, with `High` as the
/// initial quality preset (the engine's is `Medium`).
fn game() -> PathBuf {
    let root = crate::play_project::game();
    let file = format!(
        r#"{{ "build": {{ "startup_scene": "{SCENE}" }}, "quality": {{ "default": "High" }} }}"#
    );
    std::fs::write(root.join("project.rusty"), file).unwrap();
    root
}

fn session(game: &Path, lines: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_session"))
        .arg("--project")
        .arg(game)
        .arg("--empty")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the session runs");
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(lines.as_bytes()).unwrap();
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn the_session_binds_the_projects_settings() {
    let game = game();
    let replies = session(
        &game,
        "return Application.GetStartupScene()\nreturn Graphics.GetQuality()\n\
         Application.SetProductName(\"Arena\")\n",
    );
    let replies: Vec<&str> = replies.lines().collect();
    assert_eq!(replies.len(), 3, "{replies:?}");
    assert!(
        replies[0].contains(SCENE),
        "the project's scene: {}",
        replies[0]
    );
    assert!(
        replies[1].contains("High"),
        "the project's preset: {}",
        replies[1]
    );
    assert!(replies[2].contains("\"ok\":true"), "{}", replies[2]);
    let file = std::fs::read_to_string(game.join("project.rusty")).unwrap();
    assert!(
        file.contains("\"Arena\""),
        "the setter wrote the file: {file}"
    );
}

#[test]
fn play_reads_the_projects_settings() {
    let game = game();
    let dir = crate::temp::dir().join("run");
    std::fs::create_dir_all(&dir).unwrap();
    let scenario = dir.join("scenario.lua");
    let lua = format!(
        "Harness.Expect(Application.GetStartupScene() == {SCENE:?}, 'scene')\n\
         Harness.Expect(Graphics.GetQuality() == 'High', 'quality')\n\
         Application.SetProductName('Arena')\n"
    );
    std::fs::write(&scenario, lua).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_play"))
        .arg("--project")
        .arg(&game)
        .arg(&scenario)
        .arg(&dir)
        .output()
        .expect("play runs");
    let results = std::fs::read_to_string(dir.join("results.json")).unwrap();
    assert!(out.status.success(), "{results}");
    let file = std::fs::read_to_string(game.join("project.rusty")).unwrap();
    assert!(
        !file.contains("Arena"),
        "a run never writes the file: {file}"
    );
}
