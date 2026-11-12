//! A `SKILL.md`'s frontmatter, held to the limits Claude Code loads a skill by: a
//! skill outside them is skipped without a word, so the gate says it at commit
//! time instead.
//!
//! ```text
//! ---
//! name: <the folder's name: ≤ 64 characters of [a-z0-9-]>
//! description: <non-empty, ≤ 1024 characters>
//! ---
//! ```

pub const MAX_NAME: usize = 64;
pub const MAX_DESCRIPTION: usize = 1024;

/// Every problem with the frontmatter of `source`, the `SKILL.md` of `folder`, as
/// (1-based line, message).
pub fn check(folder: &str, source: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = source.lines().collect();
    let Some(end) = (lines.first().map(|l| l.trim_end()) == Some("---"))
        .then(|| lines.iter().skip(1).position(|l| l.trim_end() == "---"))
        .flatten()
        .map(|i| i + 1)
    else {
        let msg = "a `SKILL.md` opens with `---`, its `name:` and `description:`, and a \
                   closing `---`; without it Claude Code skips the skill";
        return vec![(1, msg.into())];
    };
    let block = &lines[1..end];
    let mut found = Vec::new();
    match value(block, "name") {
        None => found.push((
            1,
            "the frontmatter has no `name:`; give the folder's name".into(),
        )),
        Some((at, name)) => found.extend(name_problem(folder, &name).map(|m| (at, m))),
    }
    match value(block, "description") {
        None => found.push((
            1,
            "the frontmatter has no `description:`; say when to use it".into(),
        )),
        Some((at, d)) if d.is_empty() => found.push((
            at,
            "the `description:` is empty; say when to use the skill".into(),
        )),
        Some((at, d)) if d.chars().count() > MAX_DESCRIPTION => found.push((
            at,
            format!(
                "the `description:` is {} characters, over {MAX_DESCRIPTION}; Claude Code \
                 skips the skill: cut it to what says when to use it",
                d.chars().count()
            ),
        )),
        Some(_) => {}
    }
    found
}

fn name_problem(folder: &str, name: &str) -> Option<String> {
    let bad = |c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if name.chars().count() > MAX_NAME {
        Some(format!(
            "the `name:` is over {MAX_NAME} characters; Claude Code skips the skill"
        ))
    } else if name.is_empty() || name.chars().any(bad) {
        Some(format!(
            "the `name:` `{name}` must be only `a-z`, `0-9` and `-`; use the folder's name"
        ))
    } else if name != folder {
        Some(format!(
            "the `name:` `{name}` must equal its folder's name, `{folder}`"
        ))
    } else {
        None
    }
}

/// `key`'s value in the frontmatter `block` (whose first line is the file's line
/// 2) and the line it is on: quotes dropped, a folded (`>`) or literal (`|`) block
/// joined from its indented lines.
fn value(block: &[&str], key: &str) -> Option<(usize, String)> {
    let at = block
        .iter()
        .position(|l| l.strip_prefix(key).is_some_and(|r| r.starts_with(':')))?;
    let raw = block[at][key.len() + 1..].trim();
    let joined = if matches!(raw, ">" | "|" | ">-" | "|-" | "") {
        let more = block[at + 1..]
            .iter()
            .take_while(|l| l.starts_with(' ') || l.is_empty());
        more.map(|l| l.trim())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    } else {
        raw.to_string()
    };
    let unquoted = ['"', '\'']
        .iter()
        .find_map(|q| joined.strip_prefix(*q).and_then(|r| r.strip_suffix(*q)));
    Some((at + 2, unquoted.unwrap_or(&joined).to_string()))
}

#[cfg(test)]
#[path = "frontmatter_tests.rs"]
mod tests;
