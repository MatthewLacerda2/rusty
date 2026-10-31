//! The player boots read-only (#1653): an exported game, booted and run for a few
//! frames, leaves its folder byte-identical but for `saved/`, and the export
//! carries what the player no longer makes (a seeded script, an `.mp3`'s `.wav`).
//! Opening a project moves the working directory, so each half runs in a child
//! process (`child`), as `tests/busy_replay` does.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::core::paths::native;
use crate::core::project::{open, Access};

/// Set on the child: which half it runs (`export` or `play`).
const STEP_VAR: &str = "RUSTY_PLAYER_READ_ONLY_STEP";
/// Set on the child: the source project.
const PROJECT_VAR: &str = "RUSTY_PLAYER_READ_ONLY_PROJECT";
/// Set on the child: the export.
const OUT_VAR: &str = "RUSTY_PLAYER_READ_ONLY_OUT";

/// The one place the player may write, Unity's `persistentDataPath`.
const SAVED: &str = "saved/";

#[test]
fn an_exported_game_boots_without_writing_into_its_install() {
    let dir = crate::test_temp::dir();
    let (game, out) = (dir.join("game"), dir.join("out"));
    // A fresh project: no bundled script and no default scene yet, and an MP3.
    let audio = game.join("assets").join("audio");
    std::fs::create_dir_all(&audio).unwrap();
    std::fs::copy(mp3(), audio.join("loop.mp3")).unwrap();

    run_child("export", &game, &out);
    let shipped = tree(&out);
    for file in [
        "assets/audio/loop.wav",
        "assets/scripts/player_controller.lua",
        "assets/scenes/default.scene",
        ".seeded",
    ] {
        assert!(shipped.contains_key(file), "the export carries {file}");
    }
    let mp3_shipped = shipped.contains_key("assets/audio/loop.mp3");
    assert!(!mp3_shipped, "no MP3 ships");

    // What a seeding or a refresh would act on, were the player to run one: a
    // bundled script the game left out, and an MP3 copied in by hand.
    std::fs::remove_file(out.join(native("assets/scripts/bot.lua"))).unwrap();
    std::fs::copy(mp3(), out.join(native("assets/audio/extra.mp3"))).unwrap();
    let shipped = tree(&out);
    run_child("play", &game, &out);
    let written = differing(&shipped, &tree(&out));
    assert!(
        written.is_empty(),
        "the player wrote into its install: {written:?}"
    );
}

/// The paths outside [`SAVED`] that `after` adds, changes or removes from `before`.
fn differing(before: &Files, after: &Files) -> BTreeSet<String> {
    let paths = before.keys().chain(after.keys());
    let outside = paths.filter(|path| !path.starts_with(SAVED));
    let changed = outside.filter(|path| before.get(*path) != after.get(*path));
    changed.cloned().collect()
}

/// The audio fixture, an MP3 the refresh converts.
fn mp3() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(native("src/asset/audio/fixtures/loop.mp3"))
}

/// Run [`child`] in a new process on `step`.
fn run_child(step: &str, game: &Path, out: &Path) {
    // lint: allow(subprocess) the test binary re-runs itself: opening a project moves the working directory
    let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", "shell::player::read_only_tests::child"])
        .args(["--include-ignored", "--nocapture"])
        .env(STEP_VAR, step)
        .env(PROJECT_VAR, game)
        .env(OUT_VAR, out)
        .status()
        .expect("spawn the child process");
    assert!(status.success(), "the {step} child failed");
}

/// The child half: export the project, or boot the player's world on the export
/// and run it, as `launch` does short of the window. A no-op unless spawned.
#[test]
// lint: allow(skip) the child half: an_exported_game_boots_without_writing_into_its_install runs it
#[ignore = "run by an_exported_game_boots_without_writing_into_its_install in a child process"]
fn child() {
    let var = |name| std::env::var_os(name).map(PathBuf::from);
    let (Some(step), Some(game), Some(out)) = (var(STEP_VAR), var(PROJECT_VAR), var(OUT_VAR))
    else {
        return;
    };
    if step == Path::new("export") {
        // As the `export` binary does.
        open(&game, Access::Edit).expect("open the project");
        crate::dev::export::export(&out).expect("export the project");
        return;
    }
    let opened = open(&out, Access::Run).expect("open the export");
    let mut world = super::boot(opened.settings).expect("boot the player");
    for _ in 0..5 {
        world.tick(1.0 / 60.0);
    }
    // What the loop's exit does.
    world.resources.flush_storage();
}

/// Files by their `/`-separated relative path, with their bytes.
type Files = BTreeMap<String, Vec<u8>>;

/// Every file under `dir`.
fn tree(dir: &Path) -> Files {
    let mut files = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = crate::core::paths::to_slash(path.strip_prefix(dir).unwrap());
                files.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    files
}
