//! src/shadergen/blocks — the curated WGSL building-block library (#272).
//!
//! A [`Block`] is one composable WGSL snippet that already speaks the engine's
//! contract: it declares its tunable params (with defaults) and supplies a helper
//! function the assembler links into the chosen pass's fragment chain. The agent
//! never writes WGSL — it picks blocks by `id` and sets their params; the snippet
//! text is fixed library data, so what ships is bounded to this catalog.
//!
//! Two families, one per [`PassKind`]:
//! - **surface** (`surface`) — vary the *fragment look* of the forward pass; the
//!   standard `vs_main` + lighting are kept verbatim and each block transforms the
//!   shaded color (toon ramp, fresnel rim, emissive pulse, UV-scroll tint, …).
//! - **postfx** (`postfx`) — fullscreen effects over the tonemapped scene color (tint,
//!   vignette, scanline, grayscale, …) — the self-contained family.
//!
//! Each block's helper is a pure function with a fixed signature per family (see
//! the family modules) followed by the block's params as arguments, in catalog
//! order. The assembler emits the helper once per block id, each *instance's*
//! params as named WGSL constants, and a call per instance passing them in — so
//! the same block can appear twice in one recipe (#393).

mod postfx;
mod surface;

use super::recipe::PassKind;

/// One declared parameter of a block: its name, default value, and arity (1 for a
/// scalar, 2/3/4 for a vector). The assembler emits it per instance as a WGSL
/// constant named `<block_id>_<index>_<name>` — or, for a [`Param::runtime`] one, a
/// slot of the material's param uniform (#399) — and passes it to the helper as the
/// argument named `<name>`.
#[derive(Clone, Copy, Debug)]
pub struct Param {
    /// Param name (the recipe key and the suffix of the emitted WGSL constant).
    pub name: &'static str,
    /// Default applied when the recipe omits this param. For a vector arity the
    /// scalar default is broadcast across all lanes.
    pub default: f32,
    /// Component count: 1 scalar, or 2/3/4 for a vector literal.
    pub arity: usize,
    /// Settable per material at runtime (#399): a surface param read from the
    /// material's param uniform instead of a baked `const`, so a script can drive it
    /// (a hit flash fading, a rim glowing with shield charge). Postfx params are never
    /// runtime — a postfx pass has no material.
    pub runtime: bool,
}

impl Param {
    /// A param baked into the module as a `const`.
    pub const fn baked(name: &'static str, default: f32, arity: usize) -> Self {
        Self {
            name,
            default,
            arity,
            runtime: false,
        }
    }

    /// A [`Param::runtime`] param: read from the material's param uniform (#399).
    pub const fn live(name: &'static str, default: f32, arity: usize) -> Self {
        Self {
            name,
            default,
            arity,
            runtime: true,
        }
    }
}

/// Where in the fragment a block's call is folded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Transforms the shaded color — every postfx block and most surface ones.
    Color,
    /// Transforms the surface's UVs **before** any map is sampled (#401), so the
    /// diffuse, normal, metallic/roughness and mask lookups all move together. Its
    /// helper is `fn uv_<id>(uv: vec2<f32>, in: VertexOutput, <params…>) -> vec2<f32>`
    /// and its `call` wraps `{prev}`, the incoming UV. Surface only.
    Uv,
}

/// A curated building block: an `id`, its declared params, and the two WGSL
/// fragments the assembler weaves in — a `helper` (a top-level function
/// definition) and a `call` (an expression that applies it in the fragment
/// chain). The `call` and `helper` are fixed strings; the only thing the recipe
/// varies is the param constants the call passes in.
#[derive(Clone, Copy, Debug)]
pub struct Block {
    /// Stable id, the recipe's `BlockSel::id`.
    pub id: &'static str,
    /// One-line human description (for docs / error context).
    pub desc: &'static str,
    /// Declared params, in catalog order.
    pub params: &'static [Param],
    /// Top-level WGSL function definition this block contributes, emitted once
    /// per recipe however many instances use it. Takes the block's params as
    /// trailing arguments named after them, in [`Block::params`] order.
    pub helper: &'static str,
    /// The expression the assembler substitutes into the fragment chain. Uses
    /// `{prev}` for the incoming color so blocks chain in recipe order, and
    /// `{args}` for the instance's param constants (`, a, b`, or empty); surface
    /// blocks may also reference `in` (the `VertexOutput`), postfx blocks `uv`.
    pub call: &'static str,
    /// The extra texture slots (#400, [`crate::shadergen::textures`]) the helper
    /// samples, as `t_<slot>`. The assembler declares a slot only when some block
    /// in the recipe lists it. Empty for most blocks, and always for postfx.
    pub textures: &'static [&'static str],
    /// Which fold the call joins: the color chain or the UV chain.
    pub stage: Stage,
    /// A block that `discard`s fragments (`dissolve`) names the call that does only
    /// that, with the same `{args}` as [`Block::call`] and `in` in scope (#648). The
    /// assembler runs it in the variant's depth-only entry points too, after the UV
    /// chain, so a fragment the colour pass cuts casts no shadow and fills no SSAO
    /// depth. `None` for a block that only restyles colour, and always for postfx.
    pub cut: Option<&'static str>,
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
    fn postfx_params_are_never_runtime() {
        let runtime = catalog(PassKind::Postfx)
            .iter()
            .flat_map(|b| b.params)
            .any(|p| p.runtime);
        assert!(!runtime, "a postfx pass has no material to hold the value");
    }

    #[test]
    fn uv_stage_blocks_are_surface_only() {
        assert!(catalog(PassKind::Postfx)
            .iter()
            .all(|b| b.stage == Stage::Color));
        assert!(catalog(PassKind::Surface)
            .iter()
            .any(|b| b.stage == Stage::Uv));
    }

    #[test]
    fn only_surface_blocks_cut_and_their_helper_defines_the_cut() {
        assert!(catalog(PassKind::Postfx).iter().all(|b| b.cut.is_none()));
        for b in catalog(PassKind::Surface) {
            if let Some(cut) = b.cut {
                let name = cut.split('(').next().unwrap();
                assert!(b.helper.contains(&format!("fn {name}(")), "{}", b.id);
            }
        }
    }

    #[test]
    fn block_textures_name_real_slots_and_only_surface_blocks_have_any() {
        for b in catalog(PassKind::Surface) {
            for t in b.textures {
                assert!(crate::shadergen::textures::slot(t).is_ok(), "{}: {t}", b.id);
            }
        }
        assert!(catalog(PassKind::Postfx)
            .iter()
            .all(|b| b.textures.is_empty()));
    }

    #[test]
    fn find_resolves_known_blocks_and_rejects_unknown() {
        assert!(find(PassKind::Surface, "toon_ramp").is_some());
        assert!(find(PassKind::Postfx, "vignette").is_some());
        assert!(find(PassKind::Surface, "vignette").is_none());
        assert!(find(PassKind::Postfx, "definitely_not_a_block").is_none());
    }
}
