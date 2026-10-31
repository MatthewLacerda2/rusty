//! Which faces a material draws (#1615): URP Lit's *Render Face*, a row of the same
//! table the transparent-surface fields use (`surface.rs`), so the Lua adapter
//! (`Material.SetRenderFace` / `GetRenderFace`), `DefineAsset`'s recipe and the
//! Inspector take it with no code of their own. Kept apart from the transparent rows
//! because it applies to every material, and the Inspector shows it beside the
//! render mode.
//!
//! Pure.

use super::surface::{choice, SurfaceField, SurfaceKind, SurfaceValue};
use crate::components::RenderFace;

/// The render-face row: shown on every material, opaque or not.
pub static FACE_FIELDS: &[SurfaceField] = &[SurfaceField {
    key: "render_face",
    api: "RenderFace",
    label: "Render Face",
    kind: SurfaceKind::Choice(|| RenderFace::ALL.map(RenderFace::name).to_vec()),
    shown: |_| true,
    get: |m| SurfaceValue::Text(m.render_face.name().to_owned()),
    put: |m, v| m.render_face = choice(v, RenderFace::parse),
}];
