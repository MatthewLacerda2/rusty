//! `sanitize`'s rules: the `pub fn`s under `src/scene/rules/` a setter may store
//! through, and which of each one's float parameters it holds. A setter's float
//! counts as held only when it reaches a rule at one of those positions, so a rule
//! that passes a value through (a fallback, a bound) never vouches for it.
//!
//! A rule holds a float parameter when a statement or match arm of its body calls
//! `is_finite()` on it (on the parameter itself, a field or a chain of it; `is_nan`
//! alone lets `±inf` through) or hands it whole to another rule at a position that
//! rule holds; the positions are found to a fixed point over every rule. A rule
//! taking floats and holding none (a pass-through) is itself a finding, excused
//! with `// lint: allow(sanitize) <why>` when it holds its value another way (a
//! match that tests each arm's float); an excused rule holds its first float.

use super::finding::Finding;
use super::sanitize::{float_params, ident_after, matching, passes_at, split_top, RULE};
use crate::code::{code_lines, is_ident};

/// The folder of rules the setters must store through.
pub const HELPERS: &str = "src/scene/rules/";

/// One rule: a bare `pub fn` under [`HELPERS`], outside its file's tests.
#[derive(Clone, Debug)]
pub struct Rule {
    pub name: String,
    path: String,
    line: usize,
    /// Every parameter's name, in order.
    params: Vec<String>,
    /// The positions of its float-carrying parameters.
    floats: Vec<usize>,
    body: String,
    /// The positions of the parameters it holds.
    pub held: Vec<usize>,
}

/// The rules `source`, at `path`, defines: each bare `pub fn` before its tests.
pub fn defined(path: &str, source: &str) -> Vec<Rule> {
    let code = code_lines(source);
    let code: Vec<&str> = code.iter().map(|c| c.as_ref()).collect();
    let end = (code.iter().position(|l| l.contains("#[cfg(test)]"))).unwrap_or(code.len());
    let mut rules: Vec<Rule> = (0..end)
        .filter_map(|i| rule(path, &code[..end], i))
        .collect();
    for r in &mut rules {
        if crate::rules::allowed(source, r.line, RULE) {
            r.held.extend(r.floats.first());
        }
    }
    rules
}

fn rule(path: &str, code: &[&str], i: usize) -> Option<Rule> {
    let line = code[i].trim_start();
    if !(line.starts_with("pub fn ") || line.starts_with("pub const fn ")) {
        return None;
    }
    let name = ident_after(line, "fn ")?;
    let rest: String = code[i..].join("\n");
    let after_name = rest.find(&format!("fn {name}"))? + 3 + name.len();
    let open = after_name + rest[after_name..].find('(')?;
    let close = open + matching(&rest[open..], '(', ')')?;
    let brace = close + rest[close..].find('{')?;
    let end = brace + matching(&rest[brace..], '{', '}')?;
    let sig = &rest[open + 1..close];
    let name_of = |p: &str| -> Option<String> {
        let (n, _) = p.split_once(':')?;
        Some(n.trim().trim_start_matches("mut ").trim().to_string())
    };
    let params: Vec<String> = split_top(sig).into_iter().filter_map(name_of).collect();
    let float = float_params(sig);
    let floats = (0..params.len())
        .filter(|&k| float.contains(&params[k]))
        .collect();
    let body = rest[brace..end].to_string();
    let (path, line) = (path.to_string(), i + 1);
    Some(Rule {
        name,
        path,
        line,
        params,
        floats,
        body,
        held: Vec::new(),
    })
}

/// Every rule of `files` under [`HELPERS`], its held positions resolved.
pub fn resolved<'a>(files: impl Iterator<Item = (&'a str, &'a str)>) -> Vec<Rule> {
    let under = |p: &str| crate::modules::module_path(p).starts_with(HELPERS);
    let mut rules: Vec<Rule> = (files.filter(|(p, _)| under(p)))
        .flat_map(|(p, source)| defined(p, source))
        .collect();
    loop {
        let snapshot = rules.clone();
        let mut moved = false;
        for r in &mut rules {
            for &k in &r.floats {
                if !r.held.contains(&k) && holds(r, &r.params[k], &snapshot) {
                    r.held.push(k);
                    moved = true;
                }
            }
        }
        if !moved {
            return rules;
        }
    }
}

/// Whether `rule`'s body tests `param` itself in one statement (an `is_finite()`
/// whose receiver starts at `param`, see [`tested`]), or hands it to another of
/// `rules` at a position that one holds.
fn holds(rule: &Rule, param: &str, rules: &[Rule]) -> bool {
    rule.body
        .split([';', ',', '{', '}'])
        .any(|s| tested(s, param))
        || (rules.iter().filter(|r| r.name != rule.name))
            .any(|r| passes_at(&rule.body, &r.name, param, Some(&r.held)))
}

/// Whether `segment` calls `is_finite()` on `param`: on the parameter, a field or a
/// method chain of it (`r.min.is_finite()`, `v.iter().all(|x| x.is_finite())`, the
/// closure's argument traced back to the chain it is called on). A test on another
/// variable vouches for nothing, and neither does `is_nan` alone: it lets `±inf`
/// through.
pub(super) fn tested(segment: &str, param: &str) -> bool {
    segment.match_indices(".is_finite()").any(|(mut at, _)| {
        // A closure's argument stands for the chain the closure is passed to; each
        // step moves left, so the walk ends.
        while let Some(root) = receiver_root(segment, at) {
            let Some(bar) = segment[..at].rfind(&format!("|{root}|")) else {
                return root == param;
            };
            let Some(open) = open_paren(segment, bar) else {
                return false;
            };
            at = open;
        }
        false
    })
}

/// The identifier a method chain ending at byte `end` of `s` starts from: back over
/// `.name`, `(..)` and `[..]` groups and whitespace.
fn receiver_root(s: &str, end: usize) -> Option<&str> {
    let b = s.as_bytes();
    let mut i = end;
    loop {
        while i > 0 && b[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        while i > 0 && matches!(b[i - 1], b')' | b']') {
            i = group_start(b, i - 1)?;
        }
        let stop = i;
        while i > 0 && is_ident(char::from(b[i - 1])) {
            i -= 1;
        }
        if i == stop {
            return None;
        }
        let mut j = i;
        while j > 0 && b[j - 1].is_ascii_whitespace() {
            j -= 1;
        }
        if j == 0 || b[j - 1] != b'.' {
            return Some(&s[i..stop]);
        }
        i = j - 1;
    }
}

/// The index of the bracket opening the group that closes at `close`.
fn group_start(b: &[u8], close: usize) -> Option<usize> {
    let mut depth = 0usize;
    for k in (0..=close).rev() {
        match b[k] {
            b')' | b']' => depth += 1,
            b'(' | b'[' => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
    }
    None
}

/// The index of the unmatched `(` enclosing byte `at` of `s`.
fn open_paren(s: &str, at: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (k, c) in s.as_bytes()[..at].iter().enumerate().rev() {
        match *c {
            b')' => depth += 1,
            b'(' if depth == 0 => return Some(k),
            b'(' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// A finding for each rule taking floats and holding none of them: a pass-through.
/// One that holds its value and passes a fallback or a bound through is a rule; a
/// setter's float counts only at the value's position.
pub fn unheld(rules: &[Rule]) -> Vec<Finding> {
    let unheld = |r: &Rule| !r.floats.is_empty() && r.held.is_empty();
    (rules.iter().filter(|r| unheld(r)))
        .map(|r| Finding {
            path: r.path.clone(),
            line: r.line,
            rule: RULE,
            message: format!(
                "rule `{}` takes floats and holds none of them: test it (`<param>.is_finite()`) \
                 or hand it to a rule that does, so a setter passing through it cannot \
                 store NaN or ±inf",
                r.name
            ),
        })
        .collect()
}

#[cfg(test)]
#[path = "sanitize_rules_tests.rs"]
mod tests;
