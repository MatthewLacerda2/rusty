//! src/shadergen/blocks — the curated WGSL building-block library (#272).
//!
//! A [`Block`] is one composable WGSL snippet that already speaks the engine's
//! contract: it declares its tunable params (with defaults) and supplies a helper
//! function the assembler links into the chosen pass's fragment chain. The agent
//! never writes WGSL — it picks blocks by `id` and sets their params; the snippet
//! text is fixed library data, so what ships is bounded to this catalog.
//!
//! Two families, one per [`PassKind`](super::recipe::PassKind):
//! - **surface** ([`surface`]) — vary the *fragment look* of the forward pass; the
//!   standard `vs_main` + lighting are kept verbatim and each block transforms the
//!   shaded color (toon ramp, fresnel rim, emissive pulse, UV-scroll tint, …).
//! - **postfx** ([`postfx`]) — fullscreen effects over the HDR scene color (tint,
//!   vignette, scanline, grayscale, …) — the self-contained family.
//!
//! Each block's helper is a pure function with a fixed signature per family (see
//! the family modules); the assembler emits the params as named WGSL constants and
//! a call into the chain, so blocks compose order-independently in text.

mod postfx;
mod surface;

use super::recipe::PassKind;

/// One declared parameter of a block: its name, default value, and arity (1 for a
/// scalar, 2/3/4 for a vector). The assembler emits it as a WGSL constant named
/// `<block_id>_<name>` the block's snippet references.
#[derive(Clone, Copy, Debug)]
pub struct Param {
    /// Param name (the recipe key and the suffix of the emitted WGSL constant).
    pub name: &'static str,
    /// Default applied when the recipe omits this param. For a vector arity the
    /// scalar default is broadcast across all lanes.
    pub default: f32,
    /// Component count: 1 scalar, or 2/3/4 for a vector literal.
    pub arity: usize,
}

/// A curated building block: an `id`, its declared params, and the two WGSL
/// fragments the assembler weaves in — a `helper` (a top-level function
/// definition) and a `call` (an expression that applies it in the fragment
/// chain). The `call` and `helper` are fixed strings; the only thing the recipe
/// varies is the param constants.
#[derive(Clone, Copy, Debug)]
pub struct Block {
    /// Stable id, the recipe's `BlockSel::id`.
    pub id: &'static str,
    /// One-line human description (for docs / error context).
    pub desc: &'static str,
    /// Declared params, in catalog order.
    pub params: &'static [Param],
    /// Top-level WGSL function definition this block contributes. References its
    /// own params by the `<id>_<param>` constants the assembler emits.
    pub helper: &'static str,
    /// The expression the assembler substitutes into the fragment chain. Uses
    /// `{prev}` for the incoming color so blocks chain in recipe order; surface
    /// blocks may also reference `in` (the `VertexOutput`), postfx blocks `uv`.
    pub call: &'static str,
}

/// The catalog of blocks valid for `pass`, in stable order.
pub fn catalog(pass: PassKind) -> &'static [Block] {
    match pass {
        PassKind::Surface => surface::BLOCKS,
        PassKind::Postfx => postfx::BLOCKS,
    }
}

/// Look up a block by id within `pass`'s catalog.
pub fn find(pass: PassKind, id: &str) -> Option<&'static Block> {
    catalog(pass).iter().find(|b| b.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_block_has_unique_id_within_its_pass() {
        for pass in [PassKind::Surface, PassKind::Postfx] {
            let cat = catalog(pass);
            for (i, b) in cat.iter().enumerate() {
                assert!(
                    cat.iter().skip(i + 1).all(|o| o.id != b.id),
                    "duplicate block id {} in {}",
                    b.id,
                    pass.tag()
                );
            }
        }
    }

    #[test]
    fn find_resolves_known_blocks_and_rejects_unknown() {
        assert!(find(PassKind::Surface, "toon_ramp").is_some());
        assert!(find(PassKind::Postfx, "vignette").is_some());
        assert!(find(PassKind::Surface, "vignette").is_none());
        assert!(find(PassKind::Postfx, "definitely_not_a_block").is_none());
    }
}
