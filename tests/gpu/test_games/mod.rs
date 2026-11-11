//! Every test game under `games/` generates deterministically, passes its
//! scenarios and takes its shots (#2029), each as its own test (#2070), so
//! nextest runs them side by side and each gets its own [`BUDGET`]. Each runs the
//! `game` binary as a child, since opening a project moves the working directory.

#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "the budget times the `game` child, not the sim, with the wall clock (#757)"
)]

mod failing;

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

/// Whether a run that `took` this long is inside [`BUDGET`].
fn within_budget(took: Duration) -> bool {
    took <= BUDGET
}

fn over_budget(name: &str, took: Duration) -> String {
    format!(
        "{name}: `game --test` took {:.1} s, over its {} s budget (tests/gpu/test_games/mod.rs, \
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
pub(super) fn game_test(name: &str, games: Option<&Path>) -> Output {
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
