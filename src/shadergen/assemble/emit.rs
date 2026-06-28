//! src/shadergen/assemble/emit.rs — turn block params into WGSL text (#272).
//!
//! The small, pass-agnostic half of assembly: render a block's declared params as
//! WGSL `const`s (named `<id>_<param>`, read from the recipe or the default), and
//! fold a block chain into one expression. Kept here so [`super`] (the pass-shape
//! assemblers) stays focused on module structure. All of it is pure and
//! deterministic — the same inputs always yield the same text.

use std::fmt::Write as _;

use crate::shadergen::blocks::{Block, Param};
use crate::shadergen::recipe::BlockSel;

/// Emit one block's params as WGSL `const`s named `<id>_<param>`, reading the
/// recipe's value or the declared default.
pub fn emit_params(out: &mut String, block: &Block, sel: &BlockSel) {
    for p in block.params {
        let _ = writeln!(
            out,
            "const {}_{}: {} = {};",
            block.id,
            p.name,
            wgsl_type(p),
            literal(p, sel)
        );
    }
}

/// The WGSL type for a param's arity.
fn wgsl_type(p: &Param) -> &'static str {
    match p.arity {
        1 => "f32",
        2 => "vec2<f32>",
        3 => "vec3<f32>",
        _ => "vec4<f32>",
    }
}

/// Render a param's value as a WGSL literal: a scalar or a `vecN<f32>(...)`.
fn literal(p: &Param, sel: &BlockSel) -> String {
    let value = sel.params.get(p.name);
    if p.arity == 1 {
        let v = value.map(|v| v.scalar(p.default)).unwrap_or(p.default);
        return fmt_f32(v);
    }
    let lanes = match value {
        Some(v) => v.vector(p.arity),
        None => vec![p.default; p.arity],
    };
    let parts: Vec<String> = lanes.iter().map(|v| fmt_f32(*v)).collect();
    format!("vec{}<f32>({})", p.arity, parts.join(", "))
}

/// Format an `f32` so it is always a valid WGSL float literal (has a decimal
/// point) and is deterministic across platforms.
pub fn fmt_f32(v: f32) -> String {
    let s = format!("{v:?}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("nan") {
        s
    } else {
        format!("{s}.0")
    }
}

/// Build the chained fold expression: starting from `seed`, wrap each block's
/// `call` around the previous via its `{prev}` placeholder, in recipe order.
pub fn chain(blocks: &[(&'static Block, &BlockSel)], seed: &str) -> String {
    let mut expr = seed.to_string();
    for (block, _) in blocks {
        expr = block.call.replace("{prev}", &expr);
    }
    expr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_f32_always_emits_a_float_literal() {
        assert_eq!(fmt_f32(1.0), "1.0");
        assert_eq!(fmt_f32(0.5), "0.5");
        assert_eq!(fmt_f32(16.0), "16.0");
    }
}
