//! The play-test fails when it should (#2029): a failing scenario exits 1, a recipe
//! error exits 2 and names the recipe.

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
