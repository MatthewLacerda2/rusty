//! Every test game under `games/` generates deterministically, passes its
//! scenarios and takes its shots (#2029), each as its own test (#2070), so
//! nextest runs them side by side and each gets its own [`BUDGET`]. Each runs the
//! `game` binary as a child, since opening a project moves the working directory.

use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// How long one game's `game --test` may take at its defaults (#2070). A game
/// past it fails by name, so the game whose defaults grew is the one fixed, not
/// the whole suite run into nextest's kill.
const BUDGET: Duration = Duration::from_secs(90);

/// One `#[test]` per game, named after its folder, and a test that the list and
/// `games/` hold the same names: a folder with no line fails, naming it.
macro_rules! games {
    ($($name:ident,)*) => {
        $(
            #[test]
            fn $name() {
                passes_within_budget(stringify!($name));
            }
        )*

        #[test]
        fn every_folder_under_games_has_a_test() {
            let listed = [$(stringify!($name)),*];
            let found = rusty::dev::game::all(&rusty::dev::game::games_dir()).unwrap();
            let found: Vec<String> = found.into_iter().map(|g| g.name).collect();
            if let Some(mismatch) = mismatch(&found, &listed) {
                panic!("{mismatch}");
            }
        }
    };
}

games! {
    // sorted: one game per line, so two branches adding a game land on different lines
    arena,
    smoke,
    tetris,
    topple,
}

/// The folders under `games/` with no line, and the lines with no folder, if any.
fn mismatch(found: &[String], listed: &[&str]) -> Option<String> {
    let unlisted: Vec<&str> = found
        .iter()
        .map(String::as_str)
        .filter(|f| !listed.contains(f))
        .collect();
    let gone: Vec<&&str> = listed
        .iter()
        .filter(|l| !found.iter().any(|f| f == *l))
        .collect();
    (!unlisted.is_empty() || !gone.is_empty()).then(|| {
        format!(
            "games/ folders with no line in `games!` (add one, sorted): {unlisted:?}; \
             lines with no folder: {gone:?}"
        )
    })
}

/// `game --test` on `name` passes, proves itself deterministic and takes no more
/// than [`BUDGET`].
fn passes_within_budget(name: &str) {
    let start = Instant::now();
    let out = game_test(name, None);
    let took = start.elapsed();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{name}:\n{stdout}\n{stderr}");
    assert!(stdout.contains("ok   generating twice writes the same files"));
    assert!(within_budget(took), "{}", over_budget(name, took));
    println!("{name}: game --test took {:.1} s", took.as_secs_f64());
}

fn within_budget(took: Duration) -> bool {
    took <= BUDGET
}

fn over_budget(name: &str, took: Duration) -> String {
    format!(
        "{name}: `game --test` took {:.1} s, over its {} s budget (tests/gpu/test_games.rs, \
         BUDGET): make the game's defaults cheaper",
        took.as_secs_f64(),
        BUDGET.as_secs()
    )
}

#[test]
fn a_game_over_its_budget_fails_naming_it() {
    assert!(within_budget(BUDGET));
    let late = BUDGET + Duration::from_millis(100);
    assert!(!within_budget(late));
    let message = over_budget("slowpoke", late);
    assert!(message.starts_with("slowpoke: ") && message.contains("90.1 s, over its 90 s"));
}

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
