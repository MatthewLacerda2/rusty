//! crates/script/src/api/snapshot/mesh.rs — the Mesh component's scene-read JSON: its identity and
//! the authored values that differ from their defaults.

use serde_json::{json, Value};

use rusty_components::MeshComponent;

/// Mesh identity: the primitive kind and, for imported meshes, the
/// `path::sub_object` asset reference, plus a `Plane`'s `subdivisions` when it isn't
/// the single quad (#1524), `cast_shadows` when it isn't `On` (#1571) and the
/// small-object cull's `max_draw_distance` / `min_screen_size` when set (#1714). Never the
/// GPU geometry.
pub(crate) fn mesh_value(m: &MeshComponent) -> Value {
    let mut v = json!({ "primitive_type": m.primitive_type, "asset_ref": m.asset_ref });
    if m.subdivisions != 1 {
        v["subdivisions"] = json!(m.subdivisions);
    }
    if !m.cast_shadows.is_on() {
        v["cast_shadows"] = json!(m.cast_shadows.name());
    }
    if m.scale_in_lightmap != 1.0 {
        v["scale_in_lightmap"] = json!(m.scale_in_lightmap);
    }
    if m.max_draw_distance != 0.0 {
        v["max_draw_distance"] = json!(m.max_draw_distance);
    }
    if m.min_screen_size != 0.0 {
        v["min_screen_size"] = json!(m.min_screen_size);
    }
    v
}

super::fields::snapshot! {
    MeshComponent => mesh_value,
}
