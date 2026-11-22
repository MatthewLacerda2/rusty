//! Every test game under `games/` generates deterministically, passes its
//! scenarios and takes its shots, each as its own test, so
//! nextest runs them side by side and each gets its own [`BUDGET`]. Each runs the
//! `game` binary as a child, since opening a project moves the working directory.

#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "the hang guard times the `game` child, not the sim, with the wall clock (#757)"
)]

mod budget;
mod calibrate;
mod failing;
mod measure;

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

/// How much CPU time (user + system) one game's `game --test` may use at its
/// defaults, in yardsticks: the median CPU time of `calibrate`'s fixed slice of the
/// engine's own sim work, sampled on a test thread throughout the game's run
///. Not wall time, which a busy machine stretches, nor bare
/// CPU seconds, which a loaded machine inflates too (a shared SMT core, all-core
/// clocks, a contended cache), as it does the yardstick sampled in the same window.
/// A game past it fails by name, so the game whose defaults grew is the one fixed.
/// The heaviest game is `showroom` since `runner`'s long run was halved:
/// it reads 216 on CI's ubuntu lavapipe and 292 in a 4-vCPU lavapipe container
/// (`runner` 154 and 219; one run each, run 44394886578), so 380 is 30% over the
/// highest count and 12% under the ubuntu count doubled. On CI's macOS Metal the
/// games wait on the GPU more than they compute (`showroom` 43, `runner` 159), so
/// there the bar holds only a CPU-bound regression. Before, `runner` was the
/// heaviest at 209 to 228 on ubuntu, 223 to 240 on macOS, 238 to 249 alone on an
/// 8-core SMT desktop (RTX 2060) and 306 to 319 in the container, 1.97× with its
/// work doubled. The yardstick cancels about four fifths of a machine's load, not
/// all: beside a full-parallel cold `cargo build` the desktop read 277 to 284
/// (1.15×) where CPU time rose 1.92×. So the bar catches a regression of about 1.6×
/// on an idle machine and about 1.35× under heavy load (`docs/testing.md`, *Test
/// games*, says how to re-measure).
const BUDGET: f64 = 380.0;

/// Wall time after which a game is killed as hung, whatever its budget.
const HANG: Duration = Duration::from_secs(450);

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
    let sampler = calibrate::Sampler::start();
    let run = measure::run(&mut game_command(name, None), HANG);
    let samples = sampler.finish();
    let budget = budget::Budget {
        units: BUDGET,
        unit: calibrate::median(&samples),
    };
    let stdout = String::from_utf8_lossy(&run.output.stdout);
    let stderr = String::from_utf8_lossy(&run.output.stderr);
    if let Err(message) = budget::verdict(name, &run, &budget) {
        panic!("{message}\n{stdout}\n{stderr}");
    }
    assert!(run.output.status.success(), "{name}:\n{stdout}\n{stderr}");
    assert!(stdout.contains("ok   generating twice writes the same files"));
    println!(
        "{name}: game --test used {:.1} s of CPU in {:.1} s: {:.0} yardsticks of {:.3} s \
         ({} slices, {:.3} to {:.3} s), budget {BUDGET}",
        run.cpu.as_secs_f64(),
        run.wall.as_secs_f64(),
        budget.of(run.cpu),
        budget.unit.as_secs_f64(),
        samples.len(),
        samples.iter().min().unwrap().as_secs_f64(),
        samples.iter().max().unwrap().as_secs_f64()
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
    let mut command = crate::spawn::engine_bin!("game");
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
