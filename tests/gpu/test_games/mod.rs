//! Every test game under `games/` generates deterministically, passes its
//! scenarios and takes its shots (#2029), each as its own test (#2070), so
//! nextest runs them side by side and each gets its own [`BUDGET`]. Each runs the
//! `game` binary as a child, since opening a project moves the working directory.

#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "the hang guard times the `game` child, not the sim, with the wall clock (#757)"
)]

mod budget;
mod failing;
mod measure;

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

/// How much CPU time (user + system) one game's `game --test` may use at its
/// defaults (#2070, #2131). CPU time, not wall time: a busy machine (the nightly's
/// one-process `cargo test`) slows a game's clock, not its work. A game past it
/// fails by name, so the game whose defaults grew is the one fixed.
const BUDGET: Duration = Duration::from_secs(90);

/// How many budgets of wall time a game may take before it is killed as hung.
const HANG_FACTOR: u32 = 5;

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
            let found: Vec<_> = found.into_iter().map(|g| g.name).collect();
            if let Some(mismatch) = mismatch(&found, &listed) {
                panic!("{mismatch}");
            }
        }
    };
}

games! {
    // sorted: one game per line, so two branches adding a game land on different lines
    arena,
    fruit,
    runner,
    showroom,
    smoke,
    tetris,
    topple,
}

/// The folders under `games/` with no line, and the lines with no folder, if any.
fn mismatch(found: &[String], listed: &[&str]) -> Option<String> {
    let unlisted: Vec<_> = found
        .iter()
        .filter(|f| !listed.contains(&f.as_str()))
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

/// `game --test` on `name` passes, proves itself deterministic and stays inside
/// [`BUDGET`].
fn passes_within_budget(name: &str) {
    let run = measure::run(&mut game_command(name, None), BUDGET * HANG_FACTOR);
    let stdout = String::from_utf8_lossy(&run.output.stdout);
    let stderr = String::from_utf8_lossy(&run.output.stderr);
    if let Err(message) = budget::verdict(name, &run, BUDGET) {
        panic!("{message}\n{stdout}\n{stderr}");
    }
    assert!(run.output.status.success(), "{name}:\n{stdout}\n{stderr}");
    assert!(stdout.contains("ok   generating twice writes the same files"));
    println!(
        "{name}: game --test used {:.1} s of CPU in {:.1} s",
        run.cpu.as_secs_f64(),
        run.wall.as_secs_f64()
    );
}

/// `game --test` on `name`, generated under this test's temp dir, read from the
/// `games` folder given (the checkout's when `None`).
pub(super) fn game_test(name: &str, games: Option<&Path>) -> Output {
    game_command(name, games)
        .output()
        .expect("the game binary runs")
}

fn game_command(name: &str, games: Option<&Path>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_game"));
    command
        .arg(name)
        .arg("--test")
        .arg("--out")
        .arg(crate::temp::dir().join(name));
    if let Some(games) = games {
        command.arg("--games").arg(games);
    }
    command
}
