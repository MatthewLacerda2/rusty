//! src/shadergen/assemble/emit.rs — turn block instances into WGSL text (#272, #393).
//!
//! The small, pass-agnostic half of assembly: check a block's recipe params against
//! its declared ones (#395), render them as per-instance WGSL `const`s, emit each
//! block's helper once, and fold the chain into one expression. Kept here so
//! [`super`] (the pass-shape assemblers) stays focused on module structure. All of
//! it is pure and deterministic — the same inputs always yield the same text.
//!
//! A block is an **instance**, not a singleton (#393): its params are emitted as
//! `<id>_<index>_<param>` (`index` = its position in the recipe) and passed to the
//! shared helper as arguments, so a recipe may use the same block twice.

use std::fmt::Write as _;

use crate::shadergen::blocks::{Block, Param};
use crate::shadergen::recipe::{BlockSel, ParamValue};

/// One resolved block instance: the catalog block and the recipe entry selecting
/// it. Its position in the resolved list is its instance index.
pub type Instance<'a> = (&'static Block, &'a BlockSel);

/// Refuse a param the block doesn't declare, or one whose shape doesn't fit its
/// arity: a scalar param takes a number, a vector param takes exactly `arity`
/// numbers or one number (broadcast to every lane). Errors name the block, its
/// recipe index, the bad key and the declared params.
pub fn check_params(block: &Block, sel: &BlockSel, index: usize) -> Result<(), String> {
    for (key, value) in &sel.params {
        let at = format!("block {:?} (blocks[{index}])", block.id);
        let Some(p) = block.params.iter().find(|p| p.name == key) else {
            let declared: Vec<&str> = block.params.iter().map(|p| p.name).collect();
            return Err(format!(
                "{at}: unknown param {key:?}; declared params: {}",
                if declared.is_empty() {
                    "(none)".to_string()
                } else {
                    declared.join(", ")
                }
            ));
        };
        let fits = match value {
            ParamValue::Scalar(_) => true,
            ParamValue::Vector(v) => p.arity > 1 && v.len() == p.arity,
        };
        if !fits && p.arity == 1 {
            return Err(format!(
                "{at}: param {key:?} expects a number, got an array"
            ));
        }
        if !fits {
            return Err(format!(
                "{at}: param {key:?} expects {} numbers (or one to broadcast)",
                p.arity
            ));
        }
    }
    Ok(())
}

/// Emit every instance's params as WGSL `const`s named `<id>_<index>_<param>`,
/// reading the recipe's value or the declared default. Params must already have
/// passed [`check_params`].
pub fn emit_params(out: &mut String, instances: &[Instance]) {
    for (index, (block, sel)) in instances.iter().enumerate() {
        for p in block.params {
            let _ = writeln!(
                out,
                "const {}: {} = {};",
                const_name(block, index, p),
                wgsl_type(p),
                literal(p, sel.params.get(p.name))
            );
        }
    }
}

/// Emit each distinct block's helper once, in first-use order, however many
/// instances of it the recipe holds.
pub fn emit_helpers(out: &mut String, instances: &[Instance]) {
    for (i, (block, _)) in instances.iter().enumerate() {
        if instances[..i].iter().all(|(b, _)| b.id != block.id) {
            let _ = writeln!(out, "{}", block.helper);
        }
    }
}

/// The per-instance WGSL constant for one param.
fn const_name(block: &Block, index: usize, p: &Param) -> String {
    format!("{}_{}_{}", block.id, index, p.name)
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
fn literal(p: &Param, value: Option<&ParamValue>) -> String {
    let lanes = match value {
        Some(ParamValue::Vector(v)) => v.clone(),
        Some(ParamValue::Scalar(s)) => vec![*s; p.arity],
        None => vec![p.default; p.arity],
    };
    let parts: Vec<String> = lanes.iter().map(|v| fmt_f32(*v)).collect();
    if p.arity == 1 {
        return parts.join("");
    }
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

/// Build the chained fold expression: starting from `seed`, wrap each instance's
/// `call` around the previous via its `{prev}` placeholder, in recipe order, with
/// `{args}` filled by that instance's param constants.
pub fn chain(instances: &[Instance], seed: &str) -> String {
    let mut expr = seed.to_string();
    for (index, (block, _)) in instances.iter().enumerate() {
        let args: String = block
            .params
            .iter()
            .map(|p| format!(", {}", const_name(block, index, p)))
            .collect();
        expr = block.call.replace("{prev}", &expr).replace("{args}", &args);
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
