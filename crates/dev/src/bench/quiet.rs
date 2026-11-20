//! The quiet counters' rule (`bench::sample`): a counter beyond the shown few
//! prints only when it moved against the run it is compared with, and one line
//! counts the rest. `make bench`'s table and `bench-scenes`' both follow it.

/// Whether a quiet counter at `avg` prints against a kept run (`kept`) whose
/// average of it was `last`: it moved, or that run lacks it. With no kept run,
/// nothing to have moved against: it does not.
pub(crate) fn quiet_moved(kept: bool, last: Option<f64>, avg: f64) -> bool {
    kept && last != Some(avg)
}

/// The line counting the `unshown` quiet counters a table left out (none: no
/// line): unchanged against the kept run `against` names, as the table's header
/// does ("last": the last run; "before": `BEFORE=<dir>`'s), or with none, kept
/// for the next to compare against; all of them are in `file`.
pub(crate) fn unshown_line(unshown: usize, against: Option<&str>, file: &str) -> String {
    let since = |run| match run {
        "last" => "since the last run".to_string(),
        run => format!("against {run}"),
    };
    match (unshown, against) {
        (0, _) => String::new(),
        (n, Some(run)) => format!("({n} other counters unchanged {}: in {file})\n", since(run)),
        (n, None) => format!("({n} other counters in {file}: each prints once it changes)\n"),
    }
}
