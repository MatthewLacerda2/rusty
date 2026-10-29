//! src/procgen/recipe/op_info.rs — the shape of one [`super::OPS`] row (#411): an
//! op's tag, category, input [`Arity`] and params, and the `const` builders the
//! table is written with.

use super::arity::Arity;

/// A param's literal default value, as the agent would write it in a recipe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lit {
    Number(f32),
    Integer(u32),
    Bool(bool),
    Text(&'static str),
}

/// One param of an op: its recipe key, value type, the allowed values when the type
/// is `"enum"`, and its default (`None` = required).
#[derive(Clone, Copy, Debug)]
pub struct OpParam {
    pub name: &'static str,
    /// `number` | `integer` | `bool` | `color` (a linear RGBA array, or an sRGB
    /// `"#rrggbb"` / `"#rrggbbaa"` string) | `vec2` | `enum` | `stops` | `path` (a
    /// project-relative file path).
    pub ty: &'static str,
    /// The accepted tags for an `enum` param; empty otherwise.
    pub values: &'static [&'static str],
    pub default: Option<Lit>,
    /// Whether the param takes a link to another node as well as a number (#1494):
    /// the curated, amount-like ones, never a whole count, an enum, a colour or a
    /// size ([`super::Param`]).
    pub socket: bool,
}

/// One op: its `op` tag, menu category, the inputs it accepts, and its params.
#[derive(Clone, Copy, Debug)]
pub struct OpInfo {
    pub op: &'static str,
    /// `generator` | `color` | `vector` | `math` | `filter`.
    pub category: &'static str,
    pub inputs: Arity,
    pub params: &'static [OpParam],
}

pub(super) const fn req(name: &'static str, ty: &'static str) -> OpParam {
    OpParam {
        name,
        ty,
        values: &[],
        default: None,
        socket: false,
    }
}

pub(super) const fn opt(name: &'static str, ty: &'static str, default: Lit) -> OpParam {
    OpParam {
        name,
        ty,
        values: &[],
        default: Some(default),
        socket: false,
    }
}

/// `p`, made socketable: it takes a link as well as a number.
pub(super) const fn socket(p: OpParam) -> OpParam {
    OpParam { socket: true, ..p }
}

pub(super) const fn choice(name: &'static str, values: &'static [&'static str]) -> OpParam {
    OpParam {
        name,
        ty: "enum",
        values,
        default: None,
        socket: false,
    }
}

pub(super) const fn op(
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
