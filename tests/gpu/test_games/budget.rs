//! The budget reads CPU time, not wall time (#2131): a child that sleeps far past
//! its budget passes, one that spins past it fails, and one that hangs is killed
//! at the hang guard. The children are `sleep` and `sh`, not games, so each
//! proof costs a second or two.

use std::process::Command;
use std::time::Duration;

use super::measure;
use super::measure::Measured;
use super::HANG_FACTOR;

/// Whether a run kept to `budget` of CPU time and finished before the hang
/// guard; the message names the game and says what to do when it did not.
pub(super) fn verdict(name: &str, run: &Measured, budget: Duration) -> Result<(), String> {
    let cpu = run.cpu.as_secs_f64();
    if run.cpu > budget {
        return Err(format!(
            "{name}: `game --test` used {cpu:.1} s of CPU, over its {} s budget \
             (tests/gpu/test_games/mod.rs, BUDGET): make the game's defaults cheaper",
            budget.as_secs_f64()
        ));
    }
    if run.killed {
        return Err(format!(
            "{name}: `game --test` hung: killed after {:.1} s of wall time ({HANG_FACTOR}x \
             its budget) having used only {cpu:.1} s of CPU; it waits on something",
            run.wall.as_secs_f64()
        ));
    }
    Ok(())
}

// The proofs run `sh` and read `wait4`'s CPU time, which a sleeping child does
// not spend.
fn sh(script: &str, budget: Duration) -> Measured {
    measure::run(
        Command::new("sh").arg("-c").arg(script),
        budget * HANG_FACTOR,
    )
}

#[test]
fn a_child_that_sleeps_past_its_budget_passes() {
    let budget = Duration::from_millis(500);
    let run = sh("sleep 1.5; echo slept", budget);
    assert!(run.wall > budget, "it slept {:?}", run.wall);
    assert!(run.cpu < budget, "sleeping costs no CPU: {:?}", run.cpu);
    assert_eq!(String::from_utf8_lossy(&run.output.stdout), "slept\n");
    assert!(run.output.status.success());
    assert_eq!(verdict("sleeper", &run, budget), Ok(()));
}

#[test]
fn a_child_that_spins_past_its_budget_fails_naming_it() {
    let budget = Duration::from_millis(200);
    // A ceiling well past budget × HANG_FACTOR, so the spinner collects its 0.2 s
    // of CPU even with a fraction of a core beside two lavapipe games.
    let mut spin = Command::new("sh");
    let run = measure::run(spin.arg("-c").arg("while :; do :; done"), budget * 15);
    assert!(run.cpu > budget, "it spun only {:?}", run.cpu);
    let message = verdict("spinner", &run, budget).unwrap_err();
    assert!(message.starts_with("spinner: ") && message.contains("over its 0.2 s budget"));
    assert!(
        message.contains("make the game's defaults cheaper"),
        "{message}"
    );
}

#[test]
fn a_child_that_hangs_is_killed_at_the_guard() {
    let budget = Duration::from_millis(200);
    let run = sh("exec sleep 30", budget);
    assert!(
        run.killed && run.wall < Duration::from_secs(10),
        "{:?}",
        run.wall
    );
    assert!(!run.output.status.success());
    let message = verdict("sleepy", &run, budget).unwrap_err();
    assert!(
        message.starts_with("sleepy: ") && message.contains("hung"),
        "{message}"
    );
}
