//! src/shadergen/blocks — the curated WGSL building-block library (#272).
//!
//! A [`Block`] is one composable WGSL snippet that already speaks the engine's
//! contract: it declares its tunable params (with defaults) and supplies a helper
//! function the assembler links into the chosen pass's fragment chain. The agent
//! never writes WGSL — it picks blocks by `id` and sets their params; the snippet
//! text is fixed library data, so what ships is bounded to this catalog.
//!
//! Three families, one per [`PassKind`]:
//! - **surface** (`surface`) — vary the *fragment look* of the forward pass; the
//!   standard lighting is kept verbatim and each block transforms the shaded color
//!   (toon ramp, fresnel rim, emissive pulse, UV-scroll tint, …), moves the UVs, or
//!   moves the vertices (`vertex_waves`, #1524).
//! - **postfx** (`postfx`) — fullscreen effects over the tonemapped scene color (tint,
//!   scanline, grayscale, …) — the self-contained family; `barrel` warps where the
//!   frame is sampled (a uv stage, #1679).
//! - **ui** (`ui`, #427) — restyle one UI graphic's premultiplied colour (scanlines,
//!   glitch slices, hologram, dissolve, wipes); the rest of `ui.wgsl` is kept.
//!
//! Each block's helper is a pure function with a fixed signature per family (see
//! the family modules) followed by the block's params as arguments, in catalog
//! order. The assembler emits the helper once per block id, each *instance's*
//! params as named WGSL constants, and a call per instance passing them in — so
//! the same block can appear twice in one recipe (#393).

mod postfx;
mod surface;
mod ui;

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
    /// (a hit flash fading, a rim glowing with shield charge). A ui param is set per
    /// graphic the same way (#427), and a postfx param per post-processing volume
    /// (#671, `Graphics.SetPostParam`).
    pub runtime: bool,
    /// Per-lane defaults for a vector param whose lanes differ (`vertex_waves`'
    /// `direction`, `{1, 0.3}`); empty broadcasts [`Param::default`].
    pub lanes: &'static [f32],
    /// The names a **choice** param takes (`phosphor_mask`'s `kind`, #1679); empty for
    /// a number. A recipe names one (`kind = "slot"`), the module bakes its index as an
    /// `f32` const, and the default is index 0. A choice is always baked, scalar.
    pub choices: &'static [&'static str],
}

impl Param {
    /// A param baked into the module as a `const`.
    pub const fn baked(name: &'static str, default: f32, arity: usize) -> Self {
        Self {
            name,
            default,
            arity,
            runtime: false,
            lanes: &[],
            choices: &[],
        }
    }

    /// A [`Param::runtime`] param: read from the material's param uniform (#399).
    pub const fn live(name: &'static str, default: f32, arity: usize) -> Self {
        Self {
            name,
            default,
            arity,
            runtime: true,
            lanes: &[],
            choices: &[],
        }
    }

    /// A baked choice among `choices` (#1679): the first is the default.
    pub const fn choice(name: &'static str, choices: &'static [&'static str]) -> Self {
        Self {
            choices,
            ..Self::baked(name, 0.0, 1)
        }
    }

    /// This param with per-lane defaults, one per lane of its arity.
    pub const fn with_lanes(self, lanes: &'static [f32]) -> Self {
        Self { lanes, ..self }
    }

    /// The default value, one number per lane.
    pub fn defaults(&self) -> Vec<f32> {
        match self.lanes.is_empty() {
            true => vec![self.default; self.arity],
            false => self.lanes.to_vec(),
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
    /// and its `call` wraps `{prev}`, the incoming UV. In a postfx module (#1679,
    /// `barrel`) it moves where the frame is sampled, before anything samples it: the
    /// helper is `fn uv_<id>(uv: vec2<f32>, <params…>) -> vec2<f32>`, and a UV it sends
    /// off the frame (outside `[0, 1]`) draws black.
    Uv,
    /// Moves the vertex itself (#1524): its world position, normal and tangent, in
    /// every vertex stage that draws the mesh (forward, motion vectors, shadows). Its
    /// helper is `fn vtx_<id>(v: SurfaceVertex, time: f32, <params…>) -> SurfaceVertex`
    /// and its `call` wraps `{prev}` and passes `time`, the game time the stage draws
    /// at (last frame's for the motion vectors). Surface only.
    Vertex,
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
    /// blocks may also reference `in` (the `VertexOutput`), postfx blocks `uv` and
    /// `in` (the fullscreen `VsOut`: `in.pos` is the pixel, #1679), ui
    /// blocks `in` (the `VertexOut`) and `f` (the `UiFrag`).
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
    /// depth. `None` for a block that only restyles colour, and always for postfx and
    /// ui (a ui block fades to transparent; the UI pass has no depth passes).
    pub cut: Option<&'static str>,
    /// What the block does to the environment reflection in the lit colour, which the
    /// forward pass records for SSR to replace (#1526). Every block says, so a new one
    /// can't leave SSR painting a reflection its colour no longer holds. Postfx and ui
    /// blocks colour no lit surface: they write [`Reflection::Kept`].
    pub reflection: Reflection,
}

/// How a surface colour block changes the reflection SSR replaces (#1526). The
/// forward pass records the environment reflection it added and the weight an SSR
/// hit gets (`lighting/ssr_surface.wgsl`); a block folded in after it must scale them
/// as it scales the colour, or SSR subtracts a reflection the pixel no longer holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reflection {
    /// The block adds light of its own (a rim, a glow) or moves the UVs before any
    /// lighting: the reflection is still all there.
    Kept,
    /// The block scales the colour per channel, linearly (a tint, a `mix` toward a
    /// colour by an amount): this `vec3<f32>` expression, with `in` in scope and
    /// `{args}` filled as [`Block::call`]'s, is the factor. The record scales by it.
    Scaled(&'static str),
    /// The block is not a per-channel scale of the colour (a toon ramp, a desaturate
    /// mixing the channels): an opaque pixel gets no SSR. A `receive_ssr`
    /// transparent (#1570) still does: it traces its hit in the forward pass before
    /// the blocks run, so the block restyles the reflection with the colour.
    Dropped,
}

/// The catalog of blocks valid for `pass`, in stable order.
pub fn catalog(pass: PassKind) -> &'static [Block] {
    match pass {
        PassKind::Surface => surface::BLOCKS,
        PassKind::Postfx => postfx::BLOCKS,
        PassKind::Ui => ui::BLOCKS,
    }
}

/// Look up a block by id within `pass`'s catalog.
pub fn find(pass: PassKind, id: &str) -> Option<&'static Block> {
    catalog(pass).iter().find(|b| b.id == id)
}

#[cfg(test)]
mod tests;
