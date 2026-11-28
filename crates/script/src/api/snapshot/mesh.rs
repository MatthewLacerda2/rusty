use serde_json::{json, Value};

use rusty_components::MeshComponent;

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
    if m.receive_gi != rusty_components::ReceiveGi::default() {
        v["receive_gi"] = json!(m.receive_gi.name());
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
