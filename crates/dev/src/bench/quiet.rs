pub(crate) fn quiet_moved(kept: bool, last: Option<f64>, avg: f64) -> bool {
    kept && last != Some(avg)
}

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
