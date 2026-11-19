//! How the fold spells a crate it folded in, one line at a time: the one
//! tokenizer behind both [`super::fold::caller`]'s rewrite and the `fold-names`
//! rule (`rules/fold_names.rs`), so what the rule calls unfoldable is exactly
//! what the rewrite leaves.
//!
//! A crate is named two ways. As a path, `rusty_font::X` (or `::rusty_font::X`),
//! anywhere on the line, as the fold always has. And bare, with no `::` after it:
//! `use rusty_font as font;`, `use rusty_font;`, `extern crate rusty_font;`, a
//! name in a `use` group or in an attribute or macro argument. A bare name is
//! read in code only, never inside a string or a comment, and is not a crate when
//! it is a field or method (`.rusty_font`), a metavariable or lifetime
//! (`$rusty_font`, `'rusty_font`), a raw identifier or a macro (`rusty_font!`).
//! Both forms become what the path form does: `crate::<module>`, or `crate` for
//! the lib. Two bare forms cannot be folded, and are left as they were:
//! [`Left`].

use crate::code::{code_of, Open};

/// The names the fold rewrites in one file, and what each becomes.
pub struct Names<'a> {
    /// `<from>::…` becomes `<to>::…` wherever it is spelled: `crate`, the lib
    /// and each member.
    pub paths: Vec<(&'a str, &'a str)>,
    /// A crate named with no `::` after it, in code only: the lib and each
    /// member, never `crate`.
    pub bare: Vec<(&'a str, &'a str)>,
}

/// A crate's name the fold cannot rewrite, so the folded copy fails to compile.
#[derive(Debug, PartialEq, Eq)]
pub struct Left {
    /// The name as written, e.g. `rusty_font`.
    pub name: String,
    /// Why it cannot fold, and what to write instead.
    pub why: &'static str,
}

/// A `pub use` of a whole member.
const REEXPORT: &str = "a `pub use` of a whole crate folds into a re-export of a \
     crate-private module, which rustc refuses: re-export its items \
     (`pub use rusty_<m>::{…};`) instead";
/// The lib named by itself.
const LIB_ALONE: &str = "the lib named by itself folds to `crate`, which cannot \
     be imported alone: write a path (`<lib>::…`) or `use <lib> as <alias>;`";

/// `line` with every name in `names` rewritten in one pass (a replacement is
/// never renamed again), and every bare name it had to leave. `open` is the
/// literal or block comment open as the line starts, and is left as it ends.
pub fn rename(line: &str, open: &mut Open, names: &Names) -> (String, Vec<Left>) {
    let mut code = Vec::new();
    code_of(line, open, |at, _| code.push(at));
    let mut out = String::new();
    let mut left = Vec::new();
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        let at = line.len() - rest.len();
        let before = &line[..at];
        let hit = if before.ends_with(ident_char) {
            None
        } else if let Some(p) =
            word(rest, &names.paths).filter(|p| after(rest, p.0).starts_with("::"))
        {
            Some((p, None))
        } else if code.binary_search(&at).is_ok() && crate_position(before) {
            word(rest, &names.bare)
                .filter(|b| !after(rest, b.0).starts_with("::"))
                .filter(|b| !after(rest, b.0).trim_start().starts_with('!'))
                .map(|b| (b, unfoldable(&out, rest, b)))
        } else {
            None
        };
        match hit {
            Some(((from, _), Some(why))) => {
                left.push(Left {
                    name: from.to_string(),
                    why,
                });
                out.push_str(from);
                rest = &rest[from.len()..];
            }
            Some(((from, to), None)) => {
                if out.ends_with("::") && !out.ends_with(":::") {
                    out.truncate(out.len() - 2);
                }
                if let Some(head) = out.strip_suffix("extern crate ") {
                    out = format!("{head}use ");
                }
                out.push_str(to);
                rest = &rest[from.len()..];
            }
            None => {
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    (out, left)
}

fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The entry of `list` that `rest` starts with as a whole word.
fn word<'n>(rest: &str, list: &[(&'n str, &'n str)]) -> Option<(&'n str, &'n str)> {
    list.iter().copied().find(|(from, _)| {
        rest.strip_prefix(from)
            .is_some_and(|r| !r.starts_with(ident_char))
    })
}

fn after<'r>(rest: &'r str, from: &str) -> &'r str {
    &rest[from.len()..]
}

/// Whether a bare word after `before` can name a crate: not a field or method
/// (one `.`, where `..` is a range), a metavariable, a lifetime or a raw
/// identifier.
fn crate_position(before: &str) -> bool {
    let field = before.ends_with('.') && !before.ends_with("..");
    !(field || before.ends_with(['$', '\'', '#']))
}

/// Why the bare `name` at the head of `rest` cannot fold, after `out`.
fn unfoldable(out: &str, rest: &str, (from, to): (&str, &str)) -> Option<&'static str> {
    let head = out.trim_start();
    if head.starts_with("pub use ") && to != "crate" {
        return Some(REEXPORT);
    }
    let next = after(rest, from).trim_start();
    (to == "crate" && next.starts_with([';', ',', '}'])).then_some(LIB_ALONE)
}
