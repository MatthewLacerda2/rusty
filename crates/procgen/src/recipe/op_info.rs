use super::arity::Arity;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lit {
    Number(f32),
    Integer(u32),
    Bool(bool),
    Vec2([f32; 2]),
    Text(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct OpParam {
    pub name: &'static str,
    /// `number` | `integer` | `bool` | `color` | `vec2` | `enum` | `stops` | `path`
    /// (project-relative file) | `text` (any string).
    pub ty: &'static str,
    pub values: &'static [&'static str],
    pub default: Option<Lit>,
    /// Takes a link to another node as well as a number: only curated, amount-like params.
    pub socket: bool,
    /// A valid value for a required param whose type's generic sample would not parse
    /// (SVG path data is not any text). `None` takes the type's sample.
    pub sample: Option<&'static str>,
}

#[derive(Clone, Copy, Debug)]
pub struct OpInfo {
    pub op: &'static str,
    pub category: &'static str,
    pub inputs: Arity,
    pub params: &'static [OpParam],
}

pub(crate) const fn req(name: &'static str, ty: &'static str) -> OpParam {
    OpParam {
        name,
        ty,
        values: &[],
        default: None,
        socket: false,
        sample: None,
    }
}

pub(crate) const fn opt(name: &'static str, ty: &'static str, default: Lit) -> OpParam {
    OpParam {
        name,
        ty,
        values: &[],
        default: Some(default),
        socket: false,
        sample: None,
    }
}

pub(crate) const fn req_sampled(name: &'static str, sample: &'static str) -> OpParam {
    OpParam {
        sample: Some(sample),
        ..req(name, "text")
    }
}

pub(crate) const fn socket(p: OpParam) -> OpParam {
    OpParam { socket: true, ..p }
}

pub(crate) const fn choice(name: &'static str, values: &'static [&'static str]) -> OpParam {
    OpParam {
        name,
        ty: "enum",
        values,
        default: None,
        socket: false,
        sample: None,
    }
}

pub(crate) const fn op(
    op: &'static str,
    category: &'static str,
    inputs: Arity,
    params: &'static [OpParam],
) -> OpInfo {
    OpInfo {
        op,
        category,
        inputs,
        params,
    }
}

/// A stochastic op's own seed, folded into the recipe seed.
pub(crate) const SEED: OpParam = opt("seed", "integer", Lit::Integer(0));
