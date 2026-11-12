//! The play-test fails when it should (#2029): a failing scenario exits 1, a recipe
//! error exits 2 and names the recipe, and a scenario whose demo diverges on
//! replay fails its replay step (#2120).

use super::game_test;

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
    std::fs::write(game.join("scenarios").join("fails.lua"), scenario).unwrap();
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

/// A mover script that slides its entity by `step[1]` each tick, once a
/// scenario's handle gives it a step.
const MOVER: &str = "local M = {}\n\
    function M.Update(id)\n\
      if M.step then local x, y, z = Transform.GetPosition(id)\n\
        Transform.SetPosition(id, x + M.step[1], y, z) end\n\
    end\nreturn M\n";

/// A game whose one scenario sets the mover's step through its handle: as a
/// sequence (`{ 0.5 }`, plain data the demo records) or as a table with mixed
/// keys, which the demo cannot carry, so its replay never moves the entity.
fn mover_game(games: &std::path::Path, name: &str, step: &str) {
    let game = games.join(name);
    std::fs::create_dir_all(game.join("scenarios")).unwrap();
    std::fs::create_dir_all(game.join("scripts")).unwrap();
    std::fs::write(game.join("scripts").join("mover.lua"), MOVER).unwrap();
    let recipe = format!(
        "Scene.New()\nlocal e = Scene.CreateEntity(\"Mover\", \"Cube\")\n\
         Scene.AddScript(e, \"assets/scripts/mover.lua\")\n\
         Scene.Save(\"assets/scenes/{name}.scene\")\n\
         Application.SetStartupScene(\"assets/scenes/{name}.scene\")\n"
    );
    std::fs::write(game.join("recipe.lua"), recipe).unwrap();
    let scenario = format!(
        "Harness.Step(1)\n\
         Scene.GetScript(Scene.FindEntityByName(\"Mover\"), \"mover\").step = {step}\n\
         Harness.Step(10)\n"
    );
    std::fs::write(game.join("scenarios").join("slide.lua"), scenario).unwrap();
}

#[test]
fn a_demo_that_diverges_on_replay_fails_the_play_test() {
    let games = crate::temp::dir().join("games");
    mover_game(&games, "slides", "{ 0.5 }");
    mover_game(&games, "drifts", "{ 0.5, k = 1 }");
    let out = game_test("slides", Some(&games));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "a recorded handle write replays: {stdout}"
    );
    assert!(
        stdout.contains("ok   scenario slide's demo replays"),
        "{stdout}"
    );
    let out = game_test("drifts", Some(&games));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    assert!(
        stdout.contains("ok   scenario slide ("),
        "the scenario itself passed: {stdout}"
    );
    assert!(
        stdout.contains("FAIL scenario slide's demo replays"),
        "{stdout}"
    );
}
