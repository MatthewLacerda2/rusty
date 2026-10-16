//! The rules gate (#934): every check that reports a [`Finding`] and honours the
//! shared inline allow (`finding.rs`). A new rule is one more call in [`run`], not
//! a new flag, gate or baseline.
//!
//! Usage: `cargo run --manifest-path tools/lint/Cargo.toml -- --rules`
//! Exit code 1 on any finding; report mirrored to `.lint/report.txt`.

mod finding;
mod ignored;
mod markers;
mod prefix;
mod skip;

pub use finding::Finding;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::exit;

const REPORT: &str = ".lint/report.txt";

/// Entry point: run every rule over [`crate::size::SCAN_ROOTS`].
pub fn run() {
    let files: Vec<String> = crate::size::SCAN_ROOTS
        .iter()
        .flat_map(|root| crate::size::scan_dir(Path::new(root)))
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    let dirs = by_dir(&files);

    let mut findings: Vec<Finding> = dirs
        .iter()
        .flat_map(|(dir, names)| prefix::check_dir(dir, names))
        .collect();
    let mut sources: BTreeMap<String, String> = BTreeMap::new();
    let mut source = |path: &str| -> String {
        sources
            .entry(path.to_string())
            .or_insert_with(|| fs::read_to_string(path).unwrap_or_default())
            .clone()
    };
    for path in &files {
        let src = source(path);
        findings.extend(markers::check(path, &src));
        findings.extend(skip::check(path, &src));
    }
    findings.retain(|f| !finding::allowed(&source(&f.path), f.line, f.rule));
    for path in &files {
        findings.extend(finding::unexplained(path, &source(path)));
    }
    // After the retain: an `ignored` file has no allow (the fix is the pattern).
    findings.extend(ignored::check(".", &files));

    let body = render(&findings, &crowded(&dirs));
    fs::create_dir_all(".lint").ok();
    fs::write(REPORT, &body).ok();
    if findings.is_empty() {
        print!("{body}");
    } else {
        eprint!("{body}");
        exit(1);
    }
}

/// File names grouped by their directory.
fn by_dir(files: &[String]) -> BTreeMap<&str, Vec<&str>> {
    let mut dirs: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for f in files {
        if let Some((dir, name)) = f.rsplit_once('/') {
            dirs.entry(dir).or_default().push(name);
        }
    }
    dirs
}

/// Directories past the ≲10-files aim, most crowded first. A signal, never a failure.
fn crowded<'a>(dirs: &BTreeMap<&'a str, Vec<&str>>) -> Vec<(&'a str, usize)> {
    let mut over: Vec<(&str, usize)> = dirs
        .iter()
        .map(|(d, names)| (*d, names.iter().filter(|n| **n != "mod.rs").count()))
        .filter(|(_, n)| *n > prefix::AIM_FILES_PER_DIR)
        .collect();
    over.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    over
}

fn render(findings: &[Finding], crowded: &[(&str, usize)]) -> String {
    let mut body = String::new();
    for f in findings {
        body.push_str(&format!("{f}\n"));
    }
    if !crowded.is_empty() {
        body.push_str(&format!(
            "signal (not a failure): {} directories hold more than {} source files; \
             cluster them by responsibility when you next work there:\n",
            crowded.len(),
            prefix::AIM_FILES_PER_DIR
        ));
        for (d, n) in crowded {
            body.push_str(&format!("  {n:>3}  {d}/\n"));
        }
    }
    body.push_str(&match findings.len() {
        0 => "rules: ok\n".to_string(),
        n => format!("rules: FAILED, {n} finding(s)\n"),
    });
    body
}

#[cfg(test)]
mod ignored_tests;
#[cfg(test)]
mod lockfile_tests;
#[cfg(test)]
mod markers_tests;
#[cfg(test)]
mod prefix_tests;
#[cfg(test)]
mod skip_tests;
