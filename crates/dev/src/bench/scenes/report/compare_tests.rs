use super::super::{read_baseline, write};
use super::*;

/// A one-viewpoint report of `frame_ms`.
fn scene(frame_ms: f64) -> SceneReport {
    let mut r = SceneReport {
        scene: "pool".into(),
        views: vec![ViewKey {
            label: "spawn".into(),
            ..ViewKey::default()
        }],
        frames: 300,
        runs: 1,
        ..SceneReport::default()
    };
    r.push_value("frame_ms", frame_ms);
    r
}

fn row(avg: f64, lo: f64, hi: f64) -> Row {
    Row {
        metric: "cpu_ms".into(),
        avg,
        p95: hi,
        worst: String::new(),
        lo,
        hi,
    }
}

#[test]
fn a_change_inside_the_spread_is_unresolved_and_one_past_it_is_given() {
    let before = row(2.0, 1.9, 2.1);
    // 2% slower, but the runs of each overlap: noise, not a change.
    let inside = delta(&before, &row(2.04, 1.95, 2.15));
    assert_eq!(inside, "unresolved (inside the spread)");
    assert!(!inside.contains('%'), "no percentage for noise: {inside}");
    // 20% slower and clear of the earlier spread: a change.
    assert_eq!(delta(&before, &row(2.4, 2.3, 2.5)), "+0.400 (+20.0%)");
}

#[test]
fn without_a_spread_the_change_is_printed_as_measured() {
    // A run of one, or a one-off row (load, memory): points, never unresolved.
    assert_eq!(
        delta(&row(2.0, 2.0, 2.0), &row(3.0, 3.0, 3.0)),
        "+1.000 (+50.0%)"
    );
    assert_eq!(delta(&row(2.0, 1.0, 3.0), &row(2.0, 1.5, 2.5)), "=");
    assert_eq!(spread(&row(2.0, 2.0, 2.0)), "");
    assert_eq!(spread(&row(2.0, 1.5, 2.25)), "1.500..2.250");
}

#[test]
fn a_viewpoint_that_moved_under_its_label_is_another_shape() {
    let before = scene(2.0);
    let mut now = scene(2.0);
    now.views[0].at = [0.0, 1.7, 3.0];
    let w = now.against(Some(&before));
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].contains("viewpoint spawn at (0.00, 0.00, 0.00)"),
        "{}",
        w[0]
    );
    assert!(w[0].contains("-> spawn at (0.00, 1.70, 3.00)"), "{}", w[0]);
}

#[test]
fn a_baseline_is_read_from_a_project_folder_or_the_whole_out_dir() {
    let dir = rusty_test_support::temp::dir().join(format!("scenes_base_{}", std::process::id()));
    let report = scene(2.0);
    write(&dir.join("game-0011"), &report, "t").unwrap();
    assert_eq!(
        read_baseline(&dir, "game-0011", "pool", false),
        Some(report.clone())
    );
    let project = dir.join("game-0011");
    assert_eq!(
        read_baseline(&project, "game-0011", "pool", false),
        Some(report)
    );
    let mut now = scene(3.0);
    now.baseline = Some(project.display().to_string());
    let text = now.render(None);
    assert!(text.contains("vs before avg"), "{text}");
    assert!(
        text.contains("the baseline holds no report of this scene"),
        "{text}"
    );
}

/// The Hi-Z parts' notes (#2401): the CPU one wherever the stage ran, the GPU
/// one (and its pass count) only with GPU timing, which lavapipe lacks.
#[test]
fn the_hiz_gpu_parts_note_needs_gpu_timing() {
    let mut cpu = scene(2.0);
    cpu.push_value("render.occlusion.test", 0.5);
    let notes = cpu.notes(true);
    assert!(notes.contains("render.occlusion.<part>"), "{notes}");
    assert!(!notes.contains("gpu.occlusion.<part>"), "{notes}");
    cpu.push_value("gpu.occlusion.select", 0.25);
    assert!(cpu.notes(true).contains("6 with a clustered batch"));
}

#[test]
fn a_script_runs_baseline_is_the_script_report_beside_the_plain_one() {
    let dir = rusty_test_support::temp::dir().join(format!("scenes_sbase_{}", std::process::id()));
    let project = dir.join("game-0011");
    let plain = scene(2.0);
    let mut timed = scene(2.5);
    timed.script = true;
    write(&project, &plain, "t").unwrap();
    write(&project.join("script"), &timed, "t").unwrap();
    for at in [&dir, &project] {
        let base = |script| read_baseline(at, "game-0011", "pool", script);
        assert_eq!(base(true), Some(timed.clone()), "{}", at.display());
        assert_eq!(base(false), Some(plain.clone()), "{}", at.display());
    }
    let only_plain = dir.join("plain");
    write(&only_plain, &plain, "t").unwrap();
    let fallback = read_baseline(&only_plain, "game-0011", "pool", true);
    assert_eq!(
        fallback,
        Some(plain),
        "the other kind, when it is all there is"
    );
}
