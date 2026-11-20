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
/// line): unchanged since the kept run, or with none, kept for the next to
/// compare against; all of them are in `file`.
pub(crate) fn unshown_line(unshown: usize, kept: bool, file: &str) -> String {
    match (unshown, kept) {
        (0, _) => String::new(),
        (n, true) => format!("({n} other counters unchanged since the last run: in {file})\n"),
        (n, false) => format!("({n} other counters in {file}: each prints once it changes)\n"),
    }
}
