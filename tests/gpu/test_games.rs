//! Every test game under `games/` generates deterministically, passes its
//! scenarios and takes its shots (#2029): adding a folder is what covers a game.
//! Each runs the `game` binary as a child, since opening a project moves the
//! working directory.

use std::path::Path;
use std::process::{Command, Output};

/// `game --test` on `name`, generated under this test's temp dir, read from the
/// `games` folder given (the checkout's when `None`).
fn game_test(name: &str, games: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_game"));
    command
        .arg(name)
        .arg("--test")
        .arg("--out")
        .arg(crate::temp::dir().join(name));
    if let Some(games) = games {
        command.arg("--games").arg(games);
    }
    command.output().expect("the game binary runs")
}

#[test]
fn every_test_game_generates_deterministically_and_passes_its_play_test() {
    let games = rusty::dev::game::all(&rusty::dev::game::games_dir()).unwrap();
    assert!(
        games.iter().any(|g| g.name == "smoke"),
        "games/smoke is there"
    );
    for game in games {
        let out = game_test(&game.name, None);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{}:\n{stdout}\n{stderr}", game.name);
        assert!(stdout.contains("ok   generating twice writes the same files"));
    }
}

#[test]
fn a_failing_scenario_fails_the_play_test() {
    let games = crate::temp::dir().join("games");
    let game = games.join("broken");
    std::fs::create_dir_all(game.join("scenarios")).unwrap();
    let recipe = "Scene.New()\nScene.CreateEntity(\"Floor\", \"Plane\")\n\
                  Scene.Save(\"assets/scenes/broken.scene\")\n\
                  Application.SetStartupScene(\"assets/scenes/broken.scene\")\n";
    std::fs::write(game.join("recipe.lua"), recipe).unwrap();
    let scenario = "Harness.Step(1)\nHarness.Expect(false, \"this always fails\")\n";
    std::fs::write(game.join("scenarios/fails.lua"), scenario).unwrap();
    let out = game_test("broken", Some(&games));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("FAIL scenario fails"), "{stdout}");
    assert!(stdout.contains("ok   generating twice"), "{stdout}");
}

#[test]
fn a_recipe_error_fails_the_generation_and_names_the_recipe() {
    let games = crate::temp::dir().join("games");
    let game = games.join("raises");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::write(game.join("recipe.lua"), "error(\"no floor today\")\n").unwrap();
    let out = game_test("raises", Some(&games));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("raises: recipe.lua") && stderr.contains("no floor today"));
}
