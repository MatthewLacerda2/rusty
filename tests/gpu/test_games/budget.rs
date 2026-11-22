//! The budget reads CPU time in yardsticks (`calibrate`), not wall time
//! nor bare CPU seconds: a child that sleeps far past its budget passes,
//! one that spins past it fails, one that hangs is killed at the wall-time ceiling, and one
//! doing twice the work fails a bar its single dose meets.

use std::process::Command;
use std::time::Duration;

use super::calibrate;
use super::measure;
use super::measure::Measured;

pub(super) struct Budget {
    pub units: f64,
    pub unit: Duration,
}

impl Budget {
    pub fn of(&self, cpu: Duration) -> f64 {
        cpu.as_secs_f64() / self.unit.as_secs_f64()
    }
}

/// Whether a run kept within `budget` and finished before the wall-time ceiling.
pub(super) fn verdict(name: &str, run: &Measured, budget: &Budget) -> Result<(), String> {
    let cpu = run.cpu.as_secs_f64();
    let used = budget.of(run.cpu);
    if used > budget.units {
        return Err(format!(
            "{name}: `game --test` used {cpu:.1} s of CPU, {used:.1} yardsticks of {:.2} s, \
             over its budget of {} (tests/gpu/test_games/mod.rs, BUDGET): make the game's \
             defaults cheaper",
            budget.unit.as_secs_f64(),
            budget.units
        ));
    }
    if run.killed {
        return Err(format!(
            "{name}: `game --test` hung: killed after {:.1} s of wall time (the HANG \
             ceiling) having used only {cpu:.1} s of CPU; it waits on something",
            run.wall.as_secs_f64()
        ));
    }
    Ok(())
}

// The proofs run `sh` and read `wait4`'s CPU time, which a sleeping child does
// not spend.
fn sh(script: &str, ceiling: Duration) -> Measured {
    measure::run(Command::new("sh").arg("-c").arg(script), ceiling)
}

fn tenths() -> Budget {
    Budget {
        units: 2.0,
        unit: Duration::from_millis(100),
    }
}

#[test]
fn a_child_that_sleeps_past_its_budget_passes() {
    let budget = tenths();
    let run = sh("sleep 1.5; echo slept", Duration::from_secs(10));
    assert!(
        run.wall > Duration::from_millis(200),
        "it slept {:?}",
        run.wall
    );
    assert!(
        budget.of(run.cpu) < 2.0,
        "sleeping costs no CPU: {:?}",
        run.cpu
    );
    assert_eq!(String::from_utf8_lossy(&run.output.stdout), "slept\n");
    assert!(run.output.status.success());
    assert_eq!(verdict("sleeper", &run, &budget), Ok(()));
}

#[test]
fn a_child_that_spins_past_its_budget_fails_naming_it() {
    // 3 s, far past the 0.2 s budget: the spinner gets its CPU even on a fraction of a core.
    let run = sh("while :; do :; done", Duration::from_secs(3));
    assert!(
        run.cpu > Duration::from_millis(200),
        "it spun only {:?}",
        run.cpu
    );
    let message = verdict("spinner", &run, &tenths()).unwrap_err();
    assert!(
        message.starts_with("spinner: ") && message.contains("over its budget of 2"),
        "{message}"
    );
    assert!(
        message.contains("make the game's defaults cheaper"),
        "{message}"
    );
}

#[test]
fn a_child_that_hangs_is_killed_at_the_ceiling() {
    let run = sh("exec sleep 30", Duration::from_secs(1));
    assert!(
        run.killed && run.wall < Duration::from_secs(10),
        "{:?}",
        run.wall
    );
    assert!(!run.output.status.success());
    let message = verdict("sleepy", &run, &tenths()).unwrap_err();
    assert!(
        message.starts_with("sleepy: ") && message.contains("hung"),
        "{message}"
    );
}

#[test]
fn the_sampler_takes_slices_through_its_window() {
    let sampler = calibrate::Sampler::start();
    std::thread::sleep(calibrate::IDLE * 2);
    let slices = sampler.finish();
    assert!(slices.len() >= 3, "{} slices", slices.len());
    assert!(slices.iter().all(|s| *s > Duration::ZERO), "{slices:?}");
}

#[test]
fn the_yardstick_spends_cpu_on_this_thread() {
    let samples = calibrate::samples(5);
    assert_eq!(samples.len(), 5);
    let unit = calibrate::median(&samples);
    assert!(unit > Duration::from_millis(10), "{unit:?}");
}
