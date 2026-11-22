//! A formula is untrusted text: [`compile`] refuses anything it cannot run, and nothing can make it
//! panic, recurse past [`MAX_DEPTH`] or read past [`MAX_LEN`] characters. Functions shared with
//! `math` are `math`'s own (`ops::math::apply`), so the two bake the same bytes.

mod eval;
mod fault;
mod lex;
mod parse;

pub use eval::{paint, Env};
pub use parse::compile;

pub const MAX_LEN: usize = 4096;

pub const MAX_DEPTH: usize = 64;

pub const INPUTS: [char; 4] = ['a', 'b', 'c', 'd'];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    code: Vec<Instr>,
    stack: usize,
    inputs: [bool; 4],
}

impl Program {
    pub fn inputs(&self) -> [bool; 4] {
        self.inputs
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyntaxError {
    pub what: String,
    pub column: Option<usize>,
    pub fix: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Instr {
    Load(Load),
    Call(Func),
}

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
    K(u8),
}

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
