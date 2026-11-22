use super::finding::Finding;
use super::scan::Scan;
use crate::repo::git;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

pub const RULE: &str = "ignored";

pub fn check(dir: &str, files: &[String]) -> Vec<Finding> {
    match ignored_paths(dir, files) {
        Ok(paths) => paths.into_iter().map(finding).collect(),
        Err(why) => vec![Finding {
            path: ".gitignore".into(),
            line: 1,
            rule: RULE,
            message: format!("could not run `git check-ignore`: {why}"),
        }],
    }
}

fn finding(path: String) -> Finding {
    Finding {
        path,
        line: 1,
        rule: RULE,
        message: "`.gitignore` excludes this file, so a push may leave it out; \
                  anchor the pattern or move the file"
            .into(),
    }
}

fn ignored_paths(dir: &str, files: &[String]) -> Result<Vec<String>, String> {
    let mut child = git(Path::new(dir))
        .args(["check-ignore", "--no-index", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    stdin
        .write_all(files.join("\n").as_bytes())
        .map_err(|e| e.to_string())?;
    // Closed before the wait: git reads its list to the end, so an open pipe hangs it.
    drop(stdin);
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    // Exit 1 means nothing is ignored; only past it did git fail.
    if out.status.code().is_none_or(|c| c > 1) {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(String::from)
        .collect())
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    check(&scan.root().to_string_lossy(), &scan.files)
}

#[cfg(test)]
#[path = "ignored_tests.rs"]
mod tests;
