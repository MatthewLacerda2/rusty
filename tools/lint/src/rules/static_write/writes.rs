//! What counts as writing a `static` on one line (`test-static-write`): the
//! in-place calls, and a `.lock()` guard written through. The rule above decides
//! which lines are asked and which names are process-wide.

/// The calls that write a `static` in place: an atomic's, a `OnceLock`'s (a
/// cache filled), an `RwLock`'s write guard.
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

/// What ends the `Result` a `.lock()` returns, leaving the guard.
const GUARDS: &[&str] = &[".unwrap()", ".unwrap_or_else(", ".expect("];

/// The first call on a guard that writes through it, as prefixes: `.push`
/// covers `.push_str(` and `.push_back(`, `.pop` covers `.pop_front(`.
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

/// The identifiers on `line`, each with its byte offset.
fn idents(line: &str) -> impl Iterator<Item = (usize, &str)> {
    let base = line.as_ptr() as usize;
    line.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .map(move |w| (w.as_ptr() as usize - base, w))
}

/// Does `line` write a `static` `is_process` names, by [`WRITES`], or through
/// the guard of its `.lock()`? A path's last segment counts as the name.
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

/// What follows the guard in `rest` (just past a `.lock()`): past one
/// [`GUARDS`] call, or `rest` itself when none follows. `None` when the call's
/// argument runs past the line, so the guard cannot be seen.
fn past_guard(rest: &str) -> Option<&str> {
    let Some(call) = GUARDS.iter().find(|g| rest.starts_with(**g)) else {
        return Some(rest);
    };
    if call.ends_with(')') {
        return Some(&rest[call.len()..]);
    }
    Some(&rest[call.len() - 1 + closing(&rest[call.len() - 1..])?..])
}

/// The byte just past the `)` closing the `(` that `s` opens with.
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

/// Does `rest`, what follows a guard, write through it: a mutating first call
/// ([`MUTATES`], a one-argument `.replace(`, which `str`'s two-argument one is
/// not), a field or index assigned or mutated, or the guard itself assigned when `deref`
/// (`*X.lock().unwrap() = …`)?
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

/// Does `args` hold a comma outside any bracket: more than one argument?
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

/// Does `rest` open with an assignment (` = `, ` += `, …), not a comparison?
fn assigns(rest: &str) -> bool {
    let rest = rest.trim_start();
    [
        "=", "+=", "-=", "*=", "/=", "%=", "|=", "&=", "^=", "<<=", ">>=",
    ]
    .iter()
    .any(|op| rest.strip_prefix(op).is_some_and(|t| !t.starts_with('=')))
}
