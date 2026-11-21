//! `--comment-only [<rev>]`: proof that a branch changes only comments.
//!
//! Over the files changed between the merge base of `<rev>` (default
//! `origin/main`) and `HEAD`, each file's two versions have their comments cut
//! by the same split the cap counts with (a doc attribute is a comment there too)
//! and are compared as code, whitespace and blank lines aside, since rustfmt may
//! re-join a line once its trailing comment goes. A file added, deleted or
//! renamed, or one that is not Rust, Lua or WGSL, fails. A tool mode, not a gate:
//! a cleanup branch runs it before readying.

use super::{bare, literals, split, Kind, Line};
use crate::gates::Verdict;
use crate::repo::git;
use std::path::Path;

/// The flag that runs it, and the base it compares against by default.
pub const FLAG: &str = "--comment-only";
pub const DEFAULT_BASE: &str = "origin/main";

/// The proof over the repository at `root`, against the merge base of `rev` and
/// `HEAD`.
pub fn verdict(root: &Path, rev: &str) -> Verdict {
    let failed = |why: String| Verdict::listed("comment-only", vec![why], |_| String::new());
    let base = match run(root, &["merge-base", rev, "HEAD"]) {
        Ok(out) => out.trim().to_string(),
        Err(why) => return failed(format!("comment-only: no merge base with `{rev}`: {why}")),
    };
    let changed = match run(root, &["diff", "--name-status", "-M", &base, "HEAD"]) {
        Ok(out) => out,
        Err(why) => return failed(format!("comment-only: git diff failed: {why}")),
    };
    let (mut findings, mut fences, mut files) = (Vec::new(), 0, 0);
    for entry in changed.lines() {
        let mut parts = entry.split('\t');
        let (status, path) = (parts.next().unwrap_or(""), parts.last().unwrap_or(""));
        files += 1;
        if status != "M" {
            findings.push(format!(
                "{path}: [comment-only] {} (status {status}); a comment-only diff only edits files",
                match status.chars().next() {
                    Some('A') => "added",
                    Some('D') => "deleted",
                    Some('R') => "renamed",
                    _ => "not an edit",
                }
            ));
            continue;
        }
        let Some(kind) = Kind::of(path) else {
            findings.push(format!(
                "{path}: [comment-only] not Rust, Lua or WGSL, so its comments cannot be told \
                 from its code; leave it out of this branch"
            ));
            continue;
        };
        let show = |rev: &str| run(root, &["show", &format!("{rev}:{path}")]);
        let (Ok(old), Ok(new)) = (show(&base), show("HEAD")) else {
            findings.push(format!(
                "{path}: [comment-only] git could not show both versions"
            ));
            continue;
        };
        let (old, new) = (split(kind, &old), split(kind, &new));
        fences += fenced(&old).saturating_sub(fenced(&new));
        if let Some(line) = differs(kind, &old, &new) {
            findings.push(format!(
                "{path}:{line}: [comment-only] the code differs, not only its comments; \
                 put the code change back, or move it to a branch of its own"
            ));
        }
    }
    let mut verdict = Verdict::listed("comment-only", findings, |n| {
        format!("\n{n} file(s) change more than comments (docs/linting.md, *The comment-only proof*).\n")
    });
    verdict.body.push_str(&format!(
        "comment-only: {files} file(s) changed since {base:.12}; {fences} fenced doc example(s) \
         removed\n"
    ));
    verdict
}

/// The first line of `new` where its code stops matching `old`'s, or `None` when
/// the two are the same code.
fn differs(kind: Kind, old: &[Line], new: &[Line]) -> Option<usize> {
    let code = |lines: &[Line]| {
        let code: Vec<&str> = lines.iter().map(|l| l.code.as_str()).collect();
        literals::normalize(kind, &code.join("\n"))
    };
    let (was, now) = (code(old), code(new));
    if was == now {
        return None;
    }
    let common = was
        .bytes()
        .zip(now.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let mut seen = 0;
    for (i, line) in new.iter().enumerate() {
        seen += literals::normalize(kind, &line.code).len();
        if seen > common {
            return Some(i + 1);
        }
    }
    Some(new.len().max(1))
}

/// How many fenced examples `lines`' comments hold: a fence opens and closes one.
fn fenced(lines: &[Line]) -> usize {
    let fences = lines.iter().filter(|l| bare(&l.comment).starts_with("```"));
    fences.count() / 2
}

fn run(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = git(root)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
#[path = "proof_tests.rs"]
mod tests;
