//! Where a bare word stands, for [`super::names`]: the brackets open around it
//! (an attribute's, a macro call's, a `use` group's) and the `use` item it is in,
//! carried from one line to the next. A token scan over code, not a parser: it
//! knows a crate position (a `use` head, an attribute or macro argument) from
//! every other place a word can be, which is all the fold needs.

use super::names::Left;
use crate::code::is_ident;

/// Where a bare word stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    /// The head of a `use` item (or `extern crate`), or of an entry in its top
    /// `{…}` group: `public` when the item is a plain `pub` one.
    Head { public: bool },
    /// A whole argument of an attribute, or (`macro_call`) of a macro.
    Argument { macro_call: bool },
    /// Anywhere else: a value, a field, a parameter, a later path segment.
    Elsewhere,
}

#[derive(Debug)]
enum Kind {
    Attribute,
    Macro,
    /// A `use` group: `root` right after `use` (its entries are crates), and
    /// what a `self` entry cannot fold for, when it groups a crate's path.
    Group {
        root: bool,
        self_of: Option<(String, &'static str)>,
    },
    Plain,
}

#[derive(Debug)]
struct Frame {
    kind: Kind,
    close: char,
}

/// The `use` item being read.
#[derive(Debug)]
struct Item {
    public: bool,
    /// How many brackets were open where it began.
    base: usize,
    /// The next word starts an entry.
    start: bool,
}

#[derive(Debug, Default, PartialEq)]
enum Tok {
    #[default]
    None,
    Word(String),
    Punct(char),
    /// `!` right after a word: a macro call.
    Bang,
}

/// The scan's state through one file.
#[derive(Debug, Default)]
pub struct Scope {
    stack: Vec<Frame>,
    item: Option<Item>,
    prev: Tok,
    prev2: Tok,
    /// A crate's path just read at a `use` head, until its `::{` opens.
    watch: Option<(String, &'static str)>,
}

impl Scope {
    /// Where a word between `before` and `after` on its line stands.
    pub fn spot(&self, before: &str, after: &str) -> Spot {
        let top = self.stack.last();
        if let Some(item) = &self.item {
            let root = matches!(
                top,
                Some(Frame {
                    kind: Kind::Group { root: true, .. },
                    ..
                })
            );
            let depth = self.stack.len() - item.base;
            return match item.start && (depth == 0 || depth == 1 && root) {
                true => Spot::Head {
                    public: item.public,
                },
                false => Spot::Elsewhere,
            };
        }
        let whole = |opens: &[char], closes: &[char]| {
            before.trim_end().ends_with(opens) && after.trim_start().starts_with(closes)
        };
        if self.stack.iter().any(|f| matches!(f.kind, Kind::Attribute)) {
            if whole(&['(', ',', '='], &[')', ',', ']']) {
                return Spot::Argument { macro_call: false };
            }
        } else if let Some(
            f @ Frame {
                kind: Kind::Macro, ..
            },
        ) = top
        {
            if whole(&[opening(f.close), ','], &[',', f.close]) {
                return Spot::Argument { macro_call: true };
            }
        }
        Spot::Elsewhere
    }

    /// A word of code read; `watch` is set when it is a crate's path at a `use`
    /// head whose `::{…}` group a `self` entry cannot fold for. Returns that
    /// `self`, when it is one.
    pub fn ident(&mut self, word: &str, watch: Option<(String, &'static str)>) -> Option<Left> {
        let mut left = None;
        let base = self.stack.len();
        match word {
            "use" if self.prev != Tok::Punct('.') => {
                let public = self.prev == Tok::Word("pub".into());
                self.item = Some(Item {
                    public,
                    base,
                    start: true,
                });
            }
            "crate" if self.prev == Tok::Word("extern".into()) => {
                let public = self.prev2 == Tok::Word("pub".into());
                self.item = Some(Item {
                    public,
                    base,
                    start: true,
                });
            }
            _ => {
                if let Some(item) = &mut self.item {
                    if let (
                        true,
                        "self",
                        Some(Frame {
                            kind:
                                Kind::Group {
                                    self_of: Some((name, why)),
                                    ..
                                },
                            ..
                        }),
                    ) = (item.start, word, self.stack.last())
                    {
                        left = Some(Left {
                            name: name.clone(),
                            why,
                        });
                    }
                    item.start = false;
                }
            }
        }
        self.watch = watch;
        self.step(Tok::Word(word.to_string()));
        left
    }

    /// A char of code outside a word, `before` it on its line.
    pub fn punct(&mut self, c: char, before: &str) {
        if c.is_whitespace() {
            return;
        }
        let mut tok = Tok::Punct(c);
        match c {
            '(' | '[' | '{' => {
                let kind = if let Some(item) = &mut self.item {
                    let root = c == '{' && item.start && self.stack.len() == item.base;
                    item.start = c == '{';
                    match c {
                        '{' => Kind::Group {
                            root,
                            self_of: if root { None } else { self.watch.take() },
                        },
                        _ => Kind::Plain,
                    }
                } else if c == '['
                    && (self.prev == Tok::Punct('#')
                        || self.prev == Tok::Punct('!') && self.prev2 == Tok::Punct('#'))
                {
                    Kind::Attribute
                } else if self.prev == Tok::Bang {
                    Kind::Macro
                } else {
                    Kind::Plain
                };
                self.stack.push(Frame {
                    kind,
                    close: closing(c),
                });
            }
            ')' | ']' | '}' => {
                self.stack.pop();
                if self
                    .item
                    .as_ref()
                    .is_some_and(|i| self.stack.len() < i.base)
                {
                    self.item = None;
                }
            }
            ';' => {
                if self
                    .item
                    .as_ref()
                    .is_some_and(|i| self.stack.len() == i.base)
                {
                    self.item = None;
                }
            }
            ',' => {
                let group = matches!(
                    self.stack.last(),
                    Some(Frame {
                        kind: Kind::Group { .. },
                        ..
                    })
                );
                if let Some(item) = self.item.as_mut().filter(|_| group) {
                    item.start = true;
                }
            }
            '!' if before.ends_with(is_ident) => tok = Tok::Bang,
            _ => {}
        }
        if c != ':' {
            self.watch = None;
        }
        self.step(tok);
    }

    fn step(&mut self, tok: Tok) {
        self.prev2 = std::mem::replace(&mut self.prev, tok);
    }
}

fn closing(open: char) -> char {
    match open {
        '(' => ')',
        '[' => ']',
        _ => '}',
    }
}

fn opening(close: char) -> char {
    match close {
        ')' => '(',
        ']' => '[',
        _ => '{',
    }
}
