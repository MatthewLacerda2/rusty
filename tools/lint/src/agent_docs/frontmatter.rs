pub const MAX_NAME: usize = 64;
pub const MAX_DESCRIPTION: usize = 1024;

pub fn check(folder: &str, source: &str) -> Vec<(usize, String)> {
    let Some(lines) = block(source) else {
        let msg = "a `SKILL.md` opens with `---`, its `name:` and `description:`, and a \
                   closing `---`; without it Claude Code skips the skill";
        return vec![(1, msg.into())];
    };
    let block = &lines[..];
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

pub(super) fn block(source: &str) -> Option<Vec<&str>> {
    let lines: Vec<&str> = source.lines().collect();
    let end = (lines.first().map(|l| l.trim_end()) == Some("---"))
        .then(|| lines.iter().skip(1).position(|l| l.trim_end() == "---"))
        .flatten()?
        + 1;
    Some(lines[1..end].to_vec())
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

pub(super) fn value(block: &[&str], key: &str) -> Option<(usize, String)> {
    let at = block.iter().position(|l| {
        l.strip_prefix(key)
            .and_then(|r| r.strip_prefix(':'))
            .is_some_and(|r| r.is_empty() || r.starts_with([' ', '\t']))
    })?;
    let raw = block[at][key.len() + 1..].trim();
    let raw = if raw.starts_with(['"', '\'']) {
        raw
    } else {
        raw.split_once(" #").map_or(raw, |(v, _)| v.trim_end())
    };
    let head = if matches!(raw, ">" | "|" | ">-" | "|-") {
        ""
    } else {
        raw
    };
    let more = block[at + 1..]
        .iter()
        .take_while(|l| l.starts_with([' ', '\t']) || l.is_empty())
        .map(|l| l.trim());
    let joined = std::iter::once(head)
        .chain(more)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let unquoted = ['"', '\'']
        .iter()
        .find_map(|q| joined.strip_prefix(*q).and_then(|r| r.strip_suffix(*q)));
    Some((at + 2, unquoted.unwrap_or(&joined).to_string()))
}

#[cfg(test)]
#[path = "frontmatter_tests.rs"]
mod tests;
