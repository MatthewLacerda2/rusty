//! The editor round's rows in a scene's table (#2314): against a last run, a
//! kept baseline (`BEFORE=`), and a last run from before the round.

use super::super::report::SceneReport;
use super::tests::round;
use super::EditorRound;

fn report(round: Option<&EditorRound>) -> SceneReport {
    let mut r = SceneReport {
        scene: "yard".into(),
        runs: 2,
        frames: 300,
        size: (64, 64),
        ..SceneReport::default()
    };
    r.push_value("load_ms", 50.0);
    r.rows
        .extend(round.into_iter().flat_map(|e| e.rows.clone()));
    r
}

fn line<'a>(table: &'a str, metric: &str) -> &'a str {
    let found = table
        .lines()
        .find(|l| l.split_whitespace().next() == Some(metric));
    found.unwrap_or_else(|| panic!("{metric}: {table}"))
}

#[test]
fn the_table_prints_the_rounds_rows_with_their_change_against_a_last_run() {
    let last = report(Some(&round([2.0, 2.0], [100.0, 100.0])));
    let now = report(Some(&round([3.0, 3.0], [150.0, 150.0])));
    let table = now.render(Some(&last));
    assert!(
        line(&table, "editor.ui_ms").contains("+1.000 (+50.0%)"),
        "{table}"
    );
    assert!(line(&table, "play_enter_ms").contains("+50.0"), "{table}");
    assert!(table.contains("(editor.<row>: the editor round"), "{table}");
    assert!(
        table.contains("editor round: this adapter has no timestamp"),
        "{table}"
    );
    // Against a kept baseline (`BEFORE=`): the same column, headed so.
    let mut before = now.clone();
    before.baseline = Some("kept".into());
    let table = before.render(Some(&last));
    assert!(table.contains("vs before avg"), "{table}");
    assert!(line(&table, "editor.ui_ms").contains("+1.000"), "{table}");
}

#[test]
fn a_last_run_from_before_the_round_gives_its_rows_no_change_column() {
    let now = report(Some(&round([3.0, 3.0], [150.0, 150.0])));
    let older = report(None);
    assert!(
        now.against(Some(&older)).is_empty(),
        "same shape: no warning"
    );
    let table = now.render(Some(&older));
    for metric in ["editor.ui_ms", "play_enter_ms"] {
        let cols = line(&table, metric).split_whitespace().count();
        assert!(cols <= 4, "no change column: {}", line(&table, metric));
    }
    // A run with the round off (`EDITOR=0`) against one with it: no error either.
    assert!(older.against(Some(&now)).is_empty());
    assert!(line(&older.render(Some(&now)), "load_ms").contains('='));
}

#[test]
fn the_rounds_clock_drift_since_the_last_run_is_its_own_warning() {
    let mut last = report(Some(&round([2.0, 2.0], [1.0, 1.0])));
    let now = report(Some(&round([2.0, 2.0], [1.0, 1.0])));
    let clock = last
        .rows
        .iter_mut()
        .find(|r| r.metric == "editor.cpu_clock_pct");
    clock.unwrap().avg = 50.0;
    let said = now.against(Some(&last));
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with("editor round: the CPU clock moved"),
        "{said:?}"
    );
    assert!(!said[0].contains("compare cpu_ms"), "{said:?}");
}
