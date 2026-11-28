//! Every test game under `games/` as its own test with its own [`BUDGET`]. Each runs the
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

/// CPU budget of one game's `game --test`, in `calibrate` yardsticks. `showroom`, the
/// heaviest, reads 216 on CI's ubuntu lavapipe and 292 in a 4-vCPU container, so 380 is
/// 30% over the highest; on macOS Metal games wait on the GPU, so there it only catches a
/// CPU-bound regression (`docs/testing.md`, *Test games*, says how to re-measure).
const BUDGET: f64 = 380.0;

/// Wall time after which a game is killed as hung, whatever its budget.
const HANG: Duration = Duration::from_secs(450);

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
    city,
    fruit,
    runner,
    showroom,
    smoke,
    tetris,
    topple,
}

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
