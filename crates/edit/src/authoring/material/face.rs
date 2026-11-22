use super::field::{no_hint, SurfaceField, SurfaceKind, SurfaceValue};
use super::surface::{always, choice};
use rusty_components::RenderFace;

pub static FACE_FIELDS: &[SurfaceField] = &[SurfaceField {
    key: "render_face",
    api: "RenderFace",
    label: "Render Face",
    kind: SurfaceKind::Choice(|| RenderFace::ALL.map(RenderFace::name).to_vec()),
    shown: always,
    get: |m| SurfaceValue::Text(m.render_face.name().to_owned()),
    hint: no_hint,
    put: |m, v| m.render_face = choice(v, RenderFace::parse),
}];
