use super::finding::Finding;
use super::scan::Scan;
use crate::code::{code_of, is_ident, Open};

pub const RULE: &str = "platform-cfg";

pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let mut open = Open::None;
    let mut found = Vec::new();
    let mut start = 0;
    for (i, line) in source.lines().enumerate() {
        let mut code = vec![false; line.len()];
        code_of(line, &mut open, |at, _| code[at] = true);
        let rest = &source[start..];
        if cfg_starts(line, &code).any(|(at, attr)| names_unshipped(&rest[at..], attr)) {
            found.push(Finding {
                path: path.to_string(),
                line: i + 1,
                rule: RULE,
                message: "rusty ships for macOS and Linux only: drop the `windows` / `unix` \
                          `cfg` and write the code unconditionally, or split a real difference \
                          with `target_os = \"macos\"` / `\"linux\"` (CLAUDE.md, *Ships for \
                          macOS and Linux*)"
                    .to_string(),
            });
        }
        start += line.len() + rest[line.len()..].find('\n').map_or(0, |n| n + 1);
    }
    found
}

fn cfg_starts<'a>(line: &'a str, code: &'a [bool]) -> impl Iterator<Item = (usize, bool)> + 'a {
    line.match_indices("cfg").filter_map(move |(at, _)| {
        let before = line[..at].chars().next_back();
        if !code.get(at).copied().unwrap_or(false) || before.is_some_and(is_ident) {
            return None;
        }
        let rest = &line[at + 3..];
        let (rest, attr) = match rest.strip_prefix("_attr") {
            Some(rest) => (rest, true),
            None => (rest.strip_prefix('!').unwrap_or(rest), false),
        };
        rest.strip_prefix('(')
            .map(|body| (line.len() - body.len(), attr))
    })
}

enum Token {
    Ident(String),
    Str(String),
    Punct(char),
}

fn names_unshipped(text: &str, attr: bool) -> bool {
    let tokens = predicate(text, attr);
    tokens.iter().enumerate().any(|(i, token)| {
        let Token::Ident(key) = token else {
            return false;
        };
        match (tokens.get(i + 1), tokens.get(i + 2)) {
            (Some(Token::Punct('=')), Some(Token::Str(value))) => {
                matches!(
                    (key.as_str(), value.as_str()),
                    ("target_os", "windows") | ("target_family", "windows" | "unix")
                )
            }
            (Some(Token::Punct('=')), _) => false,
            _ => key == "windows" || key == "unix",
        }
    })
}

fn predicate(text: &str, attr: bool) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    let mut depth = 1;
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '/' if chars.peek() == Some(&'/') => {
                chars.by_ref().find(|&c| c == '\n');
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = ' ';
                chars
                    .by_ref()
                    .find(|&c| std::mem::replace(&mut last, c) == '*' && c == '/');
            }
            '"' => {
                let mut value = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => value.extend(chars.next()),
                        '"' => break,
                        c => value.push(c),
                    }
                }
                tokens.push(Token::Str(value));
            }
            c if is_ident(c) => {
                let mut word = c.to_string();
                while let Some(&c) = chars.peek().filter(|&&c| is_ident(c)) {
                    word.push(c);
                    chars.next();
                }
                tokens.push(Token::Ident(word));
            }
            c => {
                depth += match c {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                };
                if depth == 0 || (attr && depth == 1 && c == ',') {
                    break;
                }
                tokens.push(Token::Punct(c));
            }
        }
    }
    tokens
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "platform_cfg_tests.rs"]
mod tests;
