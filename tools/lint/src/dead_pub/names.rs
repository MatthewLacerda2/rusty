use super::scope::{Scope, Spot};
use crate::code::{code_of, is_ident, Open};

pub struct Names<'a> {
    pub paths: Vec<(&'a str, &'a str)>,
    pub bare: Vec<(&'a str, &'a str)>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Left {
    pub name: String,
    pub why: &'static str,
}

pub const REEXPORT: &str = "a `pub use` of a whole crate folds into a re-export of a \
     crate-private module, which rustc refuses: re-export its items \
     (`pub use rusty_<m>::{…};`) instead";
pub const LIB_ALONE: &str = "the lib named by itself folds to `crate`, which cannot \
     be imported alone: write a path (`<lib>::…`) or `use <lib> as <alias>;`";

pub struct Renamer<'a> {
    names: &'a Names<'a>,
    open: Open,
    scope: Scope,
    left: Vec<Left>,
}

impl<'a> Renamer<'a> {
    pub fn new(names: &'a Names<'a>) -> Self {
        Renamer {
            names,
            open: Open::None,
            scope: Scope::default(),
            left: Vec::new(),
        }
    }

    pub fn line(&mut self, line: &str) -> (String, Vec<Left>) {
        let mut code = Vec::new();
        code_of(line, &mut self.open, |at, _| code.push(at));
        let mut out = String::new();
        let mut at = 0;
        while let Some(c) = line[at..].chars().next() {
            let (before, rest) = line.split_at(at);
            let in_code = code.binary_search(&at).is_ok();
            if !is_ident(c) || c.is_ascii_digit() || before.ends_with(is_ident) {
                if in_code {
                    self.scope.punct(c, before);
                }
                out.push(c);
                at += c.len_utf8();
                continue;
            }
            let word = &rest[..rest.find(|c: char| !is_ident(c)).unwrap_or(rest.len())];
            let after = &rest[word.len()..];
            let text = self.word(word, before, after, in_code, &mut out);
            out.push_str(text.unwrap_or(word));
            at += word.len();
        }
        (out, std::mem::take(&mut self.left))
    }

    fn word(
        &mut self,
        word: &str,
        before: &str,
        after: &str,
        in_code: bool,
        out: &mut String,
    ) -> Option<&'a str> {
        let path = after.starts_with("::");
        let list = if path {
            &self.names.paths
        } else {
            &self.names.bare
        };
        let found = list.iter().find(|(from, _)| *from == word).map(|p| p.1);
        let spot = if in_code {
            self.scope.spot(before, after)
        } else {
            Spot::Elsewhere
        };
        let lib = word != "crate" && found == Some("crate");
        let crate_named = found.is_some() && word != "crate";
        let (why, watch) = match spot {
            Spot::Head { public } if crate_named && !path => {
                let why = if lib {
                    alone(after).then_some(LIB_ALONE)
                } else {
                    public.then_some(REEXPORT)
                };
                (why, None)
            }
            Spot::Head { public } if crate_named && after[2..].trim_start().starts_with('{') => {
                let why = if lib {
                    Some(LIB_ALONE)
                } else {
                    public.then_some(REEXPORT)
                };
                (None, why.map(|w| (word.to_string(), w)))
            }
            _ => (None, None),
        };
        if let Some(why) = why {
            self.left.push(Left {
                name: word.to_string(),
                why,
            });
        }
        if in_code {
            let found = self.scope.ident(word, watch);
            self.left.extend(found);
        }
        let to = found.filter(|_| why.is_none())?;
        if path {
            if let Some(head) = out.strip_suffix("::") {
                if head.ends_with(|c: char| is_ident(c) || c == '>') {
                    return None;
                }
                out.truncate(head.len());
            }
            return Some(to);
        }
        let rewrite = match spot {
            Spot::Head { .. } => true,
            // In a macro's argument the lib's short name is likelier a local: left as written.
            Spot::Argument { macro_call } => !(macro_call && lib),
            Spot::Elsewhere => false,
        };
        let field = before.ends_with(['.', '$', '\'', '#']) && !before.ends_with("..");
        if !rewrite || field {
            return None;
        }
        if let Some(head) = out.strip_suffix("::") {
            out.truncate(head.len());
        }
        if let Some(head) = out.strip_suffix("extern crate ") {
            *out = format!("{head}use ");
        }
        Some(to)
    }
}

pub fn alone(after: &str) -> bool {
    after.trim_start().starts_with([';', ',', '}'])
}
