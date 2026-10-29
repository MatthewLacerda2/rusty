//! `play` records its run in the project's `saved/demos/`, and `play --demo`
//! replays it, failing the run if the game no longer plays it the same.

use std::process::Command;

use crate::play_project::{game, play};

#[test]
fn play_records_a_demo_and_play_demo_replays_it() {
    let game = game();
    let (out, _) = play(&game, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let demos = rusty::dev::demo::demos_in(&game.join("saved").join("demos"));
    let [demo] = demos.as_slice() else {
        panic!("one demo per run: {demos:?}");
    };
    assert!(demo
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("arena-"));

    let out_dir = crate::temp::dir().join("replay");
    let out = Command::new(env!("CARGO_BIN_EXE_play"))
        .arg("--project")
        .arg(&game)
        .arg("--demo")
        .arg(demo)
        .arg(&out_dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let results = std::fs::read_to_string(out_dir.join("results.json")).unwrap();
    assert!(results.contains("demo: replayed 3 of 3 ticks"), "{results}");
    let demos = rusty::dev::demo::demos_in(&game.join("saved").join("demos"));
    assert_eq!(demos.len(), 1, "a replay records nothing");
}
