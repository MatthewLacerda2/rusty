//! The bake log (#1667) on a fake clock: when it prints, and what.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use super::format::{count, duration};
use super::{BakeLog, Line, Progress, Step, INTERVAL};

fn lightmaps(done: u64, total: u64, meshes: (u64, u64)) -> Progress {
    Progress {
        step: Step::Lightmaps,
        done,
        total,
        meshes: Some(meshes),
    }
}

fn reflections(done: u64, total: u64) -> Progress {
    Progress {
        step: Step::ReflectionProbes,
        done,
        total,
        meshes: None,
    }
}

/// Run `observe` against a log on a clock it can set, and return the lines.
fn logged(run: impl FnOnce(&mut BakeLog, &dyn Fn(u64))) -> Vec<Line> {
    let now = Cell::new(Duration::ZERO);
    let lines = RefCell::new(Vec::new());
    {
        let mut log = BakeLog::new(|| now.get(), |l| lines.borrow_mut().push(l));
        run(&mut log, &|secs| now.set(Duration::from_secs(secs)));
    }
    lines.into_inner()
}

#[test]
fn the_issue_line_reads_as_written() {
    let lines = logged(|log, at| {
        at(0);
        log.observe(lightmaps(0, 7_600_000, (0, 1501)));
        at(26 * 60);
        log.observe(lightmaps(3_100_000, 7_600_000, (612, 1501)));
    });
    assert_eq!(
        lines[0].text,
        "[Lighting] lightmaps 0% (0/1501 meshes, 0/7.6M texels)"
    );
    assert!(lines[0].milestone);
    // 3.1M texels in 26 min leaves 4.5M at the same rate: ~37.7 min.
    assert_eq!(
        lines[1].text,
        "[Lighting] lightmaps 40% (612/1501 meshes, 3.1M/7.6M texels), ~37 min left"
    );
    assert!(!lines[1].milestone);
}

#[test]
fn a_step_prints_at_most_once_per_interval_but_every_step_change() {
    let step = INTERVAL.as_secs();
    let lines = logged(|log, at| {
        at(0);
        log.observe(lightmaps(0, 100, (0, 2)));
        at(step - 1);
        log.observe(lightmaps(50, 100, (1, 2))); // too soon
        at(step);
        log.observe(lightmaps(60, 100, (1, 2)));
        at(step + 1);
        log.observe(reflections(0, 4)); // a new step prints at once
        log.finish();
    });
    let milestones: Vec<bool> = lines.iter().map(|l| l.milestone).collect();
    assert_eq!(milestones, [true, false, true, true]);
    assert_eq!(lines[3].text, format!("[Lighting] done in {} s", step + 1));
}

#[test]
fn a_bounce_is_its_own_step_and_its_eta_says_so() {
    let probes = |bounce, done| Progress {
        step: Step::LightProbes { bounce, max: 5 },
        done,
        total: 120,
        meshes: None,
    };
    let lines = logged(|log, at| {
        at(0);
        log.observe(probes(1, 0));
        at(60);
        log.observe(probes(1, 60));
        log.observe(probes(2, 0));
    });
    assert_eq!(
        lines[1].text,
        "[Lighting] light probes, bounce 1 of at most 5: 50% (60/120 probes), ~1 min 0 s left in this bounce"
    );
    assert!(lines[2].milestone, "a new bounce prints at once");
}

#[test]
fn no_eta_before_any_work_or_while_the_total_is_unknown() {
    let lines = logged(|log, at| {
        at(0);
        log.observe(lightmaps(0, 0, (0, 0)));
        at(30);
        log.observe(lightmaps(0, 0, (0, 0)));
        at(60);
        log.observe(lightmaps(0, 1000, (0, 3)));
    });
    assert_eq!(lines[0].text, "[Lighting] lightmaps preparing");
    // The rate starts when the total is known, so rasterizing never skews it.
    assert_eq!(
        lines[2].text,
        "[Lighting] lightmaps 0% (0/3 meshes, 0/1.0k texels)"
    );
}

#[test]
fn an_off_log_prints_nothing_and_finish_without_progress_is_silent() {
    let mut off = BakeLog::off();
    off.observe(reflections(1, 2));
    off.finish();
    assert!(!off.is_on());
    assert!(logged(|log, _| log.finish()).is_empty());
}

#[test]
fn counts_and_durations_read_short() {
    assert_eq!(
        [0, 999, 12_345, 999_949, 999_999, 7_600_000].map(count),
        ["0", "999", "12.3k", "999.9k", "1.0M", "7.6M"]
    );
    let d = |s| duration(Duration::from_secs(s));
    assert_eq!(
        [d(45), d(9 * 60 + 5), d(38 * 60 + 59), d(64 * 60 + 12)],
        ["45 s", "9 min 5 s", "38 min", "1 h 4 min"]
    );
}
