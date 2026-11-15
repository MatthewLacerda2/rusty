//! The `expression` op: one node for a per-pixel formula, Substance's *Pixel
//! Processor*. Its language is [`crate::ops::expression`]'s.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::expression::{self, compile, Env, Program, SyntaxError, INPUTS};
use crate::recipe::{arity, op, opt, socket, Arity, Lit, OpInfo, Param};

/// `expr`'s default: black, on a channel `r`, `g` or `b` leaves to it.
const ZERO: &str = "0";

/// Each pixel from a formula over its position (`u`, `v`), up to four inputs (`a`
/// to `d`) and four linkable constants (`k1` to `k4`). `expr` writes red, green and
/// blue (a grayscale image); `r`, `g` and `b` each replace it on their channel, and
/// `a_out` writes alpha, 1 when left out. The formulas are compiled when the recipe
/// is parsed, so a typo is refused before anything bakes.
///
/// The fields are private: the compiled programs are built once from them, so a
/// field set afterwards would bake the old formula. A node is made by parsing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Written")]
pub struct Expression {
    /// `None` only beside all of `r`, `g` and `b`, where it is never read, so the
    /// written form round-trips; else the written formula or [`ZERO`].
    #[serde(skip_serializing_if = "Option::is_none")]
    expr: Option<String>,
    r: String,
    g: String,
    b: String,
    a_out: String,
    k1: Param,
    k2: Param,
    k3: Param,
    k4: Param,
    #[serde(skip)]
    compiled: Compiled,
}

/// The params as the recipe writes them, before the formulas compile.
#[derive(Deserialize)]
struct Written {
    /// Whether `expr` was written decides whether it is used, not its text.
    #[serde(default)]
    expr: Option<String>,
    #[serde(default)]
    r: String,
    #[serde(default)]
    g: String,
    #[serde(default)]
    b: String,
    #[serde(default)]
    a_out: String,
    #[serde(default)]
    k1: Param,
    #[serde(default)]
    k2: Param,
    #[serde(default)]
    k3: Param,
    #[serde(default)]
    k4: Param,
}

/// The programs a node runs, and which channel reads which.
#[derive(Clone, Debug, Default, PartialEq)]
struct Compiled {
    programs: Vec<Program>,
    rgb: [usize; 3],
    alpha: Option<usize>,
    /// How many inputs the formulas name: exactly this many are wired.
    inputs: u8,
}

impl TryFrom<Written> for Expression {
    type Error = String;

    fn try_from(w: Written) -> Result<Self, String> {
        let compiled = compile_all(&w)?;
        let channels = [&w.r, &w.g, &w.b];
        let expr = match w.expr {
            None if channels.iter().all(|s| given(s)) => None,
            e => Some(e.unwrap_or_else(|| ZERO.into())),
        };
        Ok(Self {
            expr,
            r: w.r,
            g: w.g,
            b: w.b,
            a_out: w.a_out,
            k1: w.k1,
            k2: w.k2,
            k3: w.k3,
            k4: w.k4,
            compiled,
        })
    }
}

/// Every formula `w` writes, compiled; refused with the field and column of the
/// first error, or when the inputs they name skip a letter.
fn compile_all(w: &Written) -> Result<Compiled, String> {
    let channels = [("r", &w.r), ("g", &w.g), ("b", &w.b)];
    if channels.iter().all(|(_, s)| given(s)) && w.expr.is_some() {
        return Err("expr is never used: r, g and b are all written; leave it out".into());
    }
    let mut programs = Vec::new();
    let mut add = |field: &str, src: &str| -> Result<usize, String> {
        programs.push(compile(src).map_err(|e| message(field, &e))?);
        Ok(programs.len() - 1)
    };
    let mut rgb = [usize::MAX; 3];
    for (slot, (field, src)) in rgb.iter_mut().zip(channels) {
        if given(src) {
            *slot = add(field, src)?;
        }
    }
    if rgb.contains(&usize::MAX) {
        let expr = add("expr", w.expr.as_deref().unwrap_or(ZERO))?;
        rgb.iter_mut()
            .filter(|s| **s == usize::MAX)
            .for_each(|s| *s = expr);
    }
    let alpha = given(&w.a_out)
        .then(|| add("a_out", &w.a_out))
        .transpose()?;
    let named = programs.iter().fold([false; 4], |mut all, p| {
        all.iter_mut().zip(p.inputs()).for_each(|(a, n)| *a |= n);
        all
    });
    let inputs = named.iter().rposition(|&n| n).map_or(0, |last| last + 1);
    if let Some(gap) = named[..inputs].iter().position(|&n| !n) {
        let (skipped, last) = (INPUTS[gap], INPUTS[inputs - 1]);
        return Err(format!(
            "the formulas name input {last} but not {skipped}; inputs are a, b, c, d \
             in wiring order, so name {skipped} or call {last} {skipped}"
        ));
    }
    Ok(Compiled {
        programs,
        rgb,
        alpha,
        inputs: inputs as u8,
    })
}

/// Whether a channel's formula is written: a blank one is left out.
fn given(s: &str) -> bool {
    !s.trim().is_empty()
}

/// `unknown function "sine" at column 13 of expr; functions are …`.
fn message(field: &str, e: &SyntaxError) -> String {
    match e.column {
        Some(c) => format!("{} at column {c} of {field}; {}", e.what, e.fix),
        None => format!("{} in {field}; {}", e.what, e.fix),
    }
}

impl Row for Expression {
    const INFO: OpInfo = op(
        "expression",
        "math",
        arity(0, 4),
        &[
            opt("expr", "text", Lit::Text(ZERO)),
            opt("r", "text", Lit::Text("")),
            opt("g", "text", Lit::Text("")),
            opt("b", "text", Lit::Text("")),
            opt("a_out", "text", Lit::Text("")),
            socket(opt("k1", "number", Lit::Number(0.0))),
            socket(opt("k2", "number", Lit::Number(0.0))),
            socket(opt("k3", "number", Lit::Number(0.0))),
            socket(opt("k4", "number", Lit::Number(0.0))),
        ],
    );
}

impl Op for Expression {
    fn run(&self, cx: &Cx) -> Image {
        let c = &self.compiled;
        let env = Env {
            inputs: cx.inputs,
            k: [&self.k1, &self.k2, &self.k3, &self.k4].map(|k| cx.field(k)),
            size: cx.size,
        };
        expression::paint(&c.programs, c.rgb, c.alpha, &env)
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![
            ("k1", &self.k1),
            ("k2", &self.k2),
            ("k3", &self.k3),
            ("k4", &self.k4),
        ]
    }

    /// Exactly the inputs the formulas name: one named but not wired, or wired but
    /// never named, is refused.
    fn arity(&self) -> Option<Arity> {
        let n = self.compiled.inputs;
        Some(arity(n, n))
    }

    /// Which input the formulas name without a wire, or which wire they never name.
    fn arity_fix(&self, given: usize) -> Option<String> {
        let named = usize::from(self.compiled.inputs);
        let letters = |range: std::ops::Range<usize>| {
            let names: Vec<String> = INPUTS[range].iter().map(char::to_string).collect();
            names.join(", ")
        };
        Some(if given > INPUTS.len() {
            "a formula reads at most four inputs, a, b, c and d; unwire the rest".into()
        } else if given < named {
            format!(
                "the formulas name {} with no input wired for it; wire one, or take it \
                 out of the formulas",
                letters(given..named)
            )
        } else {
            format!(
                "no formula names the input wired as {}; name it in a formula, or \
                 unwire it",
                letters(named..given)
            )
        })
    }
}

#[cfg(test)]
mod graph_tests;
#[cfg(test)]
mod refuse_tests;
#[cfg(test)]
mod tests;
