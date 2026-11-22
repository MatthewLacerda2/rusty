use super::super::scenes::report::Row;
use super::super::Samples;
use super::{keep_top, Window, TOP};
use crate::stats::verbs::{Frame, Verb};

fn frame(verbs: &[(&str, f64, u32)], update_ms: f64) -> Frame {
    let verbs = verbs
        .iter()
        .map(|&(name, ms, calls)| Verb {
            name: name.into(),
            ms,
            calls,
        })
        .collect();
    let overhead_ms = 0.0;
    Frame {
        verbs,
        update_ms,
        overhead_ms,
    }
}

#[test]
fn a_verb_first_called_late_is_zero_on_the_frames_before_it() {
    let (mut window, mut samples) = (Window::default(), Samples::default());
    window.push(&mut samples, &frame(&[("A.f", 0.2, 2)], 0.2), 0.5, 1.0);
    samples.end_frame();
    window.push(&mut samples, &frame(&[("B.g", 0.1, 1)], 0.1), 0.4, 2.0);
    samples.end_frame();
    assert_eq!(samples.values("script.A.f"), [0.2, 0.0]);
    assert_eq!(
        samples.values("script.B.g"),
        [0.0, 0.2],
        "scaled by the clock"
    );
    assert_eq!(samples.values("script.B.g.calls"), [0.0, 1.0]);
    assert_eq!(samples.values("script.calls"), [2.0, 1.0]);
    let lua = samples.values("script.lua");
    assert!(
        (lua[0] - 0.3).abs() < 1e-9 && (lua[1] - 0.6).abs() < 1e-9,
        "{lua:?}"
    );
}

fn row(metric: &str, avg: f64) -> Row {
    Row::value(metric.into(), avg)
}

#[test]
fn a_scenes_verbs_keep_the_costliest_and_sum_the_rest() {
    let mut rows = vec![row("cpu.update_scripts", 1.0), row("script.lua", 0.1)];
    for n in 0..TOP + 2 {
        let ms = 0.01 * (n + 1) as f64;
        rows.push(row(&format!("script.N.v{n}"), ms));
        rows.push(row(&format!("script.N.v{n}.calls"), 3.0));
    }
    rows.push(row("script.calls", 3.0 * (TOP + 2) as f64));
    keep_top(&mut rows);
    let names: Vec<&str> = rows.iter().map(|r| r.metric.as_str()).collect();
    assert_eq!(
        names[..3],
        ["cpu.update_scripts", "script.lua", "script.calls"]
    );
    let top = format!("script.N.v{}", TOP + 1);
    assert_eq!(names[3..5], [top.as_str(), &format!("{top}.calls")]);
    let tail = &names[names.len() - 2..];
    assert_eq!(tail, ["script.other_verbs", "script.other_verbs.calls"]);
    assert_eq!(rows.len(), 3 + 2 * TOP + 2);
    let other = rows.iter().find(|r| r.metric == "script.other_verbs");
    assert!(
        (other.map_or(0.0, |r| r.avg) - 0.03).abs() < 1e-9,
        "v0 and v1 summed"
    );
}

#[test]
fn a_scene_with_few_verbs_has_no_other_row() {
    let mut rows = vec![row("script.A.f", 0.2), row("script.A.f.calls", 1.0)];
    keep_top(&mut rows);
    assert_eq!(rows.len(), 2);
}

#[test]
fn the_wrappers_own_time_is_a_row_of_its_own_and_not_luas() {
    let (mut window, mut samples) = (Window::default(), Samples::default());
    let mut timed = frame(&[("A.f", 0.2, 2)], 0.2);
    timed.overhead_ms = 0.05;
    window.push(&mut samples, &timed, 0.5, 1.0);
    samples.end_frame();
    assert_eq!(samples.values("script.overhead"), [0.05]);
    let lua = samples.values("script.lua");
    assert!((lua[0] - 0.25).abs() < 1e-9, "{lua:?}");
}
