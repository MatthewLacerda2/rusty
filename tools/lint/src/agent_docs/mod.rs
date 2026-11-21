//! `agent-docs`: the files an agent loads as instructions keep their shape.
//!
//! Every tracked `CLAUDE.md` (anywhere in the tree), every tracked file of a
//! skill under `.claude/skills/<folder>/` and every subagent definition
//! `.claude/agents/<name>.md` is held to four rules:
//!
//! - `claude-md`: a `CLAUDE.md` is at most [`MAX_CLAUDE_MD_LINES`] lines, every
//!   line counted, blank ones too: the whole file costs context on every load;
//! - `skill`: a skill's file is at most [`MAX_SKILL_FILE_LINES`] lines, its
//!   `SKILL.md` opens with the frontmatter Claude Code loads it by
//!   ([`frontmatter`]), and a skill folder without a `SKILL.md` fails;
//! - `agent`: a definition is at most [`MAX_AGENT_LINES`] lines, opens with the
//!   frontmatter Claude Code loads it by, with only keys it documents and a
//!   concrete `model:`, and every definition [`agent::PINS`] names exists with
//!   its pinned model ([`agent`]);
//! - `history`: no issue or pull request number and no date ([`history`]): an
//!   agent doc states the rule, and its story lives in the issue and PR.
//!
//! An exception is inline, with a reason: `<!-- lint: allow(<rule>) <why> -->` on
//! the line or in the comment lines right above it (`rules::allowed_in`); a length
//! cap, which belongs to no single line, is excused by an allow anywhere in the
//! file.
//!
//! It runs with the size gate (`make size`, the commit hook on staged `.md` files,
//! and CI's lint job on Markdown-only pull requests too), since what it holds is
//! Markdown, which the code gates never see.

mod agent;
mod frontmatter;
pub(crate) mod history;
#[cfg(test)]
mod tests;

use crate::repo::ls_files;
use crate::rules::{allowed_in, unexplained, Finding};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub const CLAUDE_MD: &str = "claude-md";
pub const SKILL: &str = "skill";
pub const AGENT: &str = "agent";
pub const HISTORY: &str = "history";
pub const MAX_CLAUDE_MD_LINES: usize = 300;
pub const MAX_SKILL_FILE_LINES: usize = 500;
/// A definition is the dispatched helper's whole standing brief and costs context
/// on every dispatch; its procedures belong in the skill or doc it points at.
/// Twice the longest one written so far leaves room without inviting a manual.
pub const MAX_AGENT_LINES: usize = 60;
const SKILLS: &str = ".claude/skills/";
const AGENTS: &str = ".claude/agents/";
const SKILL_MD: &str = "SKILL.md";

/// The size gate's pass over the repository at `base`: the report lines for the
/// tracked tree, or, given the commit hook's explicit files, for those that are
/// agent docs. Where git can't list the tree, nothing checked and a note saying so.
pub fn gate(base: &Path, args: &[String]) -> (Vec<String>, Option<String>) {
    let (found, note) = if args.is_empty() {
        match tree(base) {
            Ok(found) => (found, None),
            Err(why) => (
                Vec::new(),
                Some(format!(
                    "lint: git can't list the tracked agent docs, so they go unchecked\n\
                     git said:\n{why}"
                )),
            ),
        }
    } else {
        (files(base, args), None)
    };
    (found.iter().map(ToString::to_string).collect(), note)
}

/// Every finding over the tracked agent docs of the repository at `base`; `Err`
/// with git's message when git cannot list them.
pub fn tree(base: &Path) -> Result<Vec<Finding>, String> {
    let spec = [":(glob)**/CLAUDE.md", ".claude/skills", ".claude/agents"];
    let tracked: Vec<String> = ls_files(base, &[], &spec)?
        .into_iter()
        .filter(|p| base.join(p).is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| is_agent_doc(p))
        .collect();
    let mut found = missing_skill_md(&tracked);
    found.extend(missing_pins(&tracked));
    for path in &tracked {
        found.extend(file(path, &read(&base.join(path))));
    }
    Ok(found)
}

/// Every finding in the explicit `paths` (the commit hook's staged files,
/// relative to the repository at `base`) that are agent docs; the rest are left
/// to the size gate.
pub fn files(base: &Path, paths: &[String]) -> Vec<Finding> {
    paths
        .iter()
        .map(|p| p.trim_start_matches("./").to_string())
        .filter(|p| is_agent_doc(p))
        .flat_map(|p| file(&p, &read(&base.join(&p))))
        .collect()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// Is the repo-relative `path` a `CLAUDE.md`, a file of a skill's folder or a
/// subagent definition?
pub fn is_agent_doc(path: &str) -> bool {
    path == "CLAUDE.md"
        || path.ends_with("/CLAUDE.md")
        || skill_of(path).is_some()
        || agent_of(path).is_some()
}

/// The definition `path` is, by its stem: `.claude/agents/<stem>.md`.
fn agent_of(path: &str) -> Option<&str> {
    let stem = path.strip_prefix(AGENTS)?.strip_suffix(".md")?;
    (!stem.is_empty() && !stem.contains('/')).then_some(stem)
}

/// The skill folder `path` belongs to: `.claude/skills/<folder>/…`.
fn skill_of(path: &str) -> Option<&str> {
    let (folder, rest) = path.strip_prefix(SKILLS)?.split_once('/')?;
    (!folder.is_empty() && !rest.is_empty()).then_some(folder)
}

/// Every finding in one agent doc, `path` repo-relative, its text `source`.
pub fn file(path: &str, source: &str) -> Vec<Finding> {
    let finding = |line: usize, rule: &'static str, message: String| Finding {
        path: path.to_string(),
        line,
        rule,
        message,
    };
    let mut found = Vec::new();
    let lines = source.lines().count();
    let cap = match (skill_of(path), agent_of(path)) {
        (Some(_), _) => (SKILL, MAX_SKILL_FILE_LINES),
        (_, Some(_)) => (AGENT, MAX_AGENT_LINES),
        _ => (CLAUDE_MD, MAX_CLAUDE_MD_LINES),
    };
    if lines > cap.1 && !excused_anywhere(path, source, cap.0) {
        let msg = format!(
            "{lines}/{} lines, every line counted; move a procedure into the skill or \
             doc that owns it",
            cap.1
        );
        found.push(finding(cap.1 + 1, cap.0, msg));
    }
    if let Some(folder) = skill_of(path).filter(|_| path.ends_with(&format!("/{SKILL_MD}"))) {
        for (line, msg) in frontmatter::check(folder, source) {
            if !allowed_in(path, source, line, SKILL) {
                found.push(finding(line, SKILL, msg));
            }
        }
    }
    for (line, msg) in agent_of(path)
        .map(|s| agent::check(s, source))
        .unwrap_or_default()
    {
        if !allowed_in(path, source, line, AGENT) {
            found.push(finding(line, AGENT, msg));
        }
    }
    for (line, msg) in history::check(source) {
        if !allowed_in(path, source, line, HISTORY) {
            found.push(finding(line, HISTORY, msg));
        }
    }
    found.extend(unexplained(path, source));
    found
}

/// Does an allow for `rule`, with its reason, sit anywhere in `source`?
fn excused_anywhere(path: &str, source: &str, rule: &str) -> bool {
    source.lines().any(|line| allowed_in(path, line, 1, rule))
}

/// A skill folder with tracked files but no tracked `SKILL.md` loads as nothing.
fn missing_skill_md(tracked: &[String]) -> Vec<Finding> {
    let folders: BTreeSet<&str> = tracked.iter().filter_map(|p| skill_of(p)).collect();
    folders
        .into_iter()
        .filter(|f| !tracked.contains(&format!("{SKILLS}{f}/{SKILL_MD}")))
        .map(|f| Finding {
            path: format!("{SKILLS}{f}/"),
            line: 1,
            rule: SKILL,
            message: format!(
                "a skill folder with no `{SKILL_MD}` is never loaded; add one with its \
                 frontmatter, or move these files into the skill that uses them"
            ),
        })
        .collect()
}

/// A definition the pin table names but the tree does not track: the doctrine's
/// helper is gone, and its dispatch falls back to a general agent.
fn missing_pins(tracked: &[String]) -> Vec<Finding> {
    agent::PINS
        .iter()
        .filter(|(name, _)| !tracked.contains(&format!("{AGENTS}{name}.md")))
        .map(|(name, model)| Finding {
            path: format!("{AGENTS}{name}.md"),
            line: 1,
            rule: AGENT,
            message: format!(
                "`agent_docs::agent::PINS` pins `{name}` to `model: {model}`, but no \
                 tracked `{AGENTS}{name}.md` defines it; restore it, or drop its row with \
                 the doctrine that names it"
            ),
        })
        .collect()
}
