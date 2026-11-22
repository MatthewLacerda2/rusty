const WRITES: &[&str] = &[
    ".store(",
    ".swap(",
    ".fetch_",
    ".compare_exchange",
    ".set(",
    ".get_or_init(",
    ".get_or_try_init(",
    ".write()",
];

/// What ends the `Result` a `.lock()` returns, leaving its guard.
const GUARDS: &[&str] = &[".unwrap()", ".unwrap_or_else(", ".expect("];

/// Prefixes of a guard's first writing call: `.push` covers `.push_str(` and `.push_back(`.
const MUTATES: &[&str] = &[
    ".push",
    ".pop",
    ".insert(",
    ".clear(",
    ".remove",
    ".retain",
    ".extend",
    ".append(",
    ".truncate(",
    ".drain(",
    ".entry(",
    ".get_mut(",
    ".iter_mut(",
    ".sort",
    ".dedup",
    ".resize(",
    ".swap(",
    ".take()",
];

fn idents(line: &str) -> impl Iterator<Item = (usize, &str)> {
    let base = line.as_ptr() as usize;
    line.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .map(move |w| (w.as_ptr() as usize - base, w))
}

pub fn writes(line: &str, is_process: &dyn Fn(&str) -> bool) -> bool {
    idents(line).any(|(at, name)| {
        let upper = name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if !upper || !is_process(name) {
            return false;
        }
        let after = &line[at + name.len()..];
        if WRITES.iter().any(|w| after.starts_with(w)) {
            return true;
        }
        let Some(guard) = after.strip_prefix(".lock()").and_then(past_guard) else {
            return false;
        };
        let path =
            line[..at].trim_end_matches(|c: char| c.is_alphanumeric() || c == '_' || c == ':');
        through_guard(guard, path.trim_end().ends_with('*'))
    })
}

fn past_guard(rest: &str) -> Option<&str> {
    let Some(call) = GUARDS.iter().find(|g| rest.starts_with(**g)) else {
        return Some(rest);
    };
    if call.ends_with(')') {
        return Some(&rest[call.len()..]);
    }
    Some(&rest[call.len() - 1 + closing(&rest[call.len() - 1..])?..])
}

fn closing(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    s.char_indices().find_map(|(i, c)| {
        depth += match c {
            '(' | '[' | '{' => 1,
            ')' | ']' | '}' => -1,
            _ => 0,
        };
        (depth == 0).then_some(i + c.len_utf8())
    })
}

fn through_guard(rest: &str, deref: bool) -> bool {
    if MUTATES.iter().any(|m| rest.starts_with(m)) {
        return true;
    }
    if let Some(args) = rest.strip_prefix(".replace").filter(|a| a.starts_with('(')) {
        return closing(args).is_some_and(|end| !top_level_comma(&args[1..end - 1]));
    }
    if let Some(field) = rest.strip_prefix('.') {
        let tail = field.trim_start_matches(|c: char| c.is_alphanumeric() || c == '_');
        if tail.len() == field.len() || tail.starts_with('(') {
            return false;
        }
        return assigns(tail) || (tail.starts_with('.') && through_guard(tail, false));
    }
    if rest.starts_with('[') {
        return closing(rest).is_some_and(|end| assigns(&rest[end..]));
    }
    deref && assigns(rest)
}

fn top_level_comma(args: &str) -> bool {
    let mut depth = 0i32;
    args.chars().any(|c| {
        depth += match c {
            '(' | '[' | '{' => 1,
            ')' | ']' | '}' => -1,
            _ => 0,
        };
        c == ',' && depth == 0
    })
}

fn assigns(rest: &str) -> bool {
    let rest = rest.trim_start();
    [
        "=", "+=", "-=", "*=", "/=", "%=", "|=", "&=", "^=", "<<=", ">>=",
    ]
    .iter()
    .any(|op| rest.strip_prefix(op).is_some_and(|t| !t.starts_with('=')))
}
