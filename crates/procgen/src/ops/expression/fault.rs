use super::lex::Tok;
use super::{SyntaxError, FUNCS};

pub(super) fn unexpected(tok: Tok, column: usize, wanted: &str) -> SyntaxError {
    let what = match tok {
        Tok::Num(n) => format!("unexpected number {n}"),
        Tok::Name(n) => format!("unexpected name {n:?}"),
        Tok::Sym(s) => format!("unexpected {s:?}"),
        Tok::End => "the formula ends early".into(),
    };
    SyntaxError {
        what,
        column: Some(column),
        fix: format!("expected {wanted}"),
    }
}

pub(super) fn show(tok: Tok) -> String {
    match tok {
        Tok::Name(n) => format!("{n:?}"),
        Tok::Num(n) => format!("{n}"),
        Tok::Sym(s) => format!("{s:?}"),
        Tok::End => "(the formula ends)".into(),
    }
}

pub(super) fn args_hint(n: usize) -> &'static str {
    ["", "x", "x, y", "x, y, z"][n.min(3)]
}

pub(super) fn unknown_name(name: &str, column: usize) -> SyntaxError {
    let fix = "names are u, v, pi, tau, the inputs a, b, c, d (each with .r .g .b .a) \
               and k1, k2, k3, k4";
    SyntaxError {
        what: format!("unknown name {name:?}"),
        column: Some(column),
        fix: fix.into(),
    }
}

pub(super) fn func_names() -> String {
    let names: Vec<&str> = FUNCS.iter().map(|(n, _)| *n).collect();
    names.join(", ")
}
