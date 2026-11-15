//! crates/procgen/src/ops/expression/mod.rs — the `expression` op's formula language:
//! one per-pixel formula, Substance's *Pixel Processor*.
//!
//! A formula is text from a recipe, so it is untrusted: [`compile`] reads it once,
//! when the recipe is parsed, into a small postfix [`Program`], and refuses anything
//! it cannot run with an error naming the column and the fix. Nothing it is handed
//! can make it panic, recurse past [`MAX_DEPTH`] or read past [`MAX_LEN`]
//! characters. [`paint`] then runs the programs at every pixel, rows split into
//! bands like every op.
//!
//! The functions it shares with the `math` op are `math`'s own
//! (`ops::math::apply`), so a formula and the equivalent `math` chain bake the
//! same bytes by construction.

mod eval;
mod fault;
mod lex;
mod parse;

pub use eval::{paint, Env};
pub use parse::compile;

/// The longest formula, in characters: a longer one is split across nodes.
pub const MAX_LEN: usize = 4096;

/// How deep parentheses, calls, unary minus and `^` may nest.
pub const MAX_DEPTH: usize = 64;

/// The inputs a formula may name, in wiring order.
pub const INPUTS: [char; 4] = ['a', 'b', 'c', 'd'];

/// A compiled formula: postfix code over a value stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    code: Vec<Instr>,
    /// The most values the stack holds while the code runs.
    stack: usize,
    /// Which of `a`–`d` the formula names.
    inputs: [bool; 4],
}

impl Program {
    /// Which of the inputs `a`, `b`, `c`, `d` the formula names.
    pub fn inputs(&self) -> [bool; 4] {
        self.inputs
    }
}

/// Why a formula did not compile: what is wrong, the 1-based column (character, not
/// byte) it was found at when it has one, and the fix.
#[derive(Clone, Debug, PartialEq)]
pub struct SyntaxError {
    pub what: String,
    pub column: Option<usize>,
    pub fix: String,
}

/// One step of a [`Program`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum Instr {
    Load(Load),
    Call(Func),
}

/// A value a formula reads.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Load {
    Num(f32),
    U,
    V,
    /// Input `slot`'s channel (0 red … 3 alpha).
    Input {
        slot: u8,
        channel: u8,
    },
    /// The socket param `k1`–`k4`, by index.
    K(u8),
}

/// An operator or function, popping its [`Func::arity`] values and pushing one.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Func {
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    Min,
    Max,
    Abs,
    Floor,
    Ceil,
    Round,
    Fract,
    Mod,
    Sqrt,
    Exp,
    Log,
    Sin,
    Cos,
    Tan,
    Atan2,
    Clamp,
    Mix,
    Step,
    Smoothstep,
    Sign,
}

/// The named functions, as a formula calls them.
const FUNCS: &[(&str, Func)] = &[
    ("min", Func::Min),
    ("max", Func::Max),
    ("abs", Func::Abs),
    ("floor", Func::Floor),
    ("ceil", Func::Ceil),
    ("round", Func::Round),
    ("fract", Func::Fract),
    ("mod", Func::Mod),
    ("sqrt", Func::Sqrt),
    ("pow", Func::Pow),
    ("exp", Func::Exp),
    ("log", Func::Log),
    ("sin", Func::Sin),
    ("cos", Func::Cos),
    ("tan", Func::Tan),
    ("atan2", Func::Atan2),
    ("clamp", Func::Clamp),
    ("mix", Func::Mix),
    ("step", Func::Step),
    ("smoothstep", Func::Smoothstep),
    ("sign", Func::Sign),
];

#[cfg(test)]
mod eval_tests;
#[cfg(test)]
mod parse_tests;
#[cfg(test)]
mod zero_tests;
