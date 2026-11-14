//! A subagent definition's frontmatter (`.claude/agents/<name>.md`), held to what
//! Claude Code loads it by, and to the models the orchestration rule pins: the
//! harness ignores a key it does not know without a word, and a definition with
//! no concrete `model:` follows the orchestrator's model again.
//!
//! ```text
//! ---
//! name: <the file's stem>
//! description: <non-empty>
//! model: <opus | sonnet | haiku | fable | a full `claude-…` model id>
//! <any other key Claude Code documents for a subagent file>
//! ---
//! ```

use super::frontmatter::{block, value};

/// Every key Claude Code documents for a subagent file
/// (<https://code.claude.com/docs/en/sub-agents>, *Supported frontmatter fields*).
pub const KEYS: &[&str] = &[
    "name",
    "description",
    "tools",
    "disallowedTools",
    "model",
    "permissionMode",
    "maxTurns",
    "autoCompactWindow",
    "skills",
    "mcpServers",
    "hooks",
    "memory",
    "background",
    "omitClaudeMd",
    "effort",
    "isolation",
    "color",
    "initialPrompt",
    "experimental",
];

/// The model aliases a definition may pin; a full model id starts `claude-`.
pub const ALIASES: &[&str] = &["opus", "sonnet", "haiku", "fable"];

/// The pins the orchestration rule names (root `CLAUDE.md`, *Coding
/// parallelises*; `issue-batch`, *Who runs on what*): definition → model. Who
/// writes or judges code is changed here, as a visible edit, not by one word in a
/// Markdown file. A definition not listed needs a concrete model, any one.
pub const PINS: &[(&str, &str)] = &[("coder", "opus"), ("reviewer", "opus")];

/// Every problem with the frontmatter of `source`, the definition whose file stem
/// is `stem`, as (1-based line, message).
pub fn check(stem: &str, source: &str) -> Vec<(usize, String)> {
    let Some(lines) = block(source) else {
        let msg = "a subagent definition opens with `---`, its `name:`, `description:` \
                   and `model:`, and a closing `---`; without it Claude Code skips it";
        return vec![(1, msg.into())];
    };
    let mut found: Vec<(usize, String)> = unknown_keys(&lines).collect();
    match value(&lines, "name") {
        None => found.push((
            1,
            "the frontmatter has no `name:`; give the file's stem".into(),
        )),
        Some((at, n)) if n != stem => found.push((
            at,
            format!("the `name:` `{n}` must equal its file's stem, `{stem}`"),
        )),
        Some(_) => {}
    }
    match value(&lines, "description") {
        Some((_, d)) if !d.is_empty() => {}
        other => found.push((
            other.map_or(1, |(at, _)| at),
            "the `description:` is missing or empty; say when to dispatch it".into(),
        )),
    }
    found.extend(model_problem(stem, value(&lines, "model")));
    found
}

/// A top-level key (a line opening with neither a space nor `#`) that no subagent
/// file documents.
fn unknown_keys<'a>(lines: &'a [&'a str]) -> impl Iterator<Item = (usize, String)> + 'a {
    lines.iter().enumerate().filter_map(|(i, l)| {
        if l.is_empty() || l.starts_with([' ', '\t', '#', '-']) {
            return None;
        }
        let key = l.split_once(':').map_or(*l, |(k, _)| k).trim();
        (!KEYS.contains(&key)).then(|| {
            (
                i + 2,
                format!(
                    "`{key}` is no key Claude Code reads in a subagent file, so it is \
                     ignored without a word; fix its spelling or drop it (the keys: \
                     `agent_docs::agent::KEYS`)"
                ),
            )
        })
    })
}

fn model_problem(stem: &str, model: Option<(usize, String)>) -> Option<(usize, String)> {
    let Some((at, m)) = model else {
        return Some((
            1,
            "the frontmatter has no `model:`, so it runs on whatever dispatched it; pin \
             one (`opus`, `sonnet`, `haiku`, `fable` or a full model id)"
                .into(),
        ));
    };
    if !(ALIASES.contains(&m.as_str()) || m.starts_with("claude-")) {
        return Some((
            at,
            format!(
                "`model: {m}` is no concrete model (`inherit` follows the orchestrator); \
                 pin `opus`, `sonnet`, `haiku`, `fable` or a full model id"
            ),
        ));
    }
    let pin = PINS.iter().find(|(name, _)| *name == stem)?.1;
    (m != pin).then(|| {
        (
            at,
            format!(
                "`{stem}` is pinned to `model: {pin}` (`agent_docs::agent::PINS`), the \
                 alias and not a full id, so it follows the latest model; change who \
                 runs it in that table, with the doctrine, not here"
            ),
        )
    })
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
