//! src/editor/inspector/components/material/shader_params.rs — the Material card's runtime
//! shader params (#399): one row per param the material's surface shader exposes at
//! runtime, `Material.SetShaderParam`'s twin.
//!
//! A thin egui client like the rest of the card: it reads each value through
//! `authoring::material::shader_param` (the stored value, else the baked default) and
//! writes through `set_shader_param`, the op the Lua binding calls.

use std::collections::BTreeMap;

use crate::scene::authoring::material as mat_ops;
use crate::scene::MaterialAsset;

/// The runtime-param rows for material `key`; nothing when its shader has none (or
/// it names no baked shader).
pub(super) fn draw_shader_params(
    ui: &mut egui::Ui,
    materials: &mut BTreeMap<String, MaterialAsset>,
    key: &str,
    is_dirty: &mut bool,
) {
    let Ok(layout) = mat_ops::shader_layout(materials, key) else {
        return;
    };
    for name in layout.names() {
        let Ok(mut value) = mat_ops::shader_param(materials, key, &layout, &name) else {
            continue;
        };
        ui.horizontal(|ui| {
            ui.label(format!("{name}:"));
            let mut changed = false;
            for lane in &mut value {
                changed |= ui.add(egui::DragValue::new(lane).speed(0.01)).changed();
            }
            if changed && mat_ops::set_shader_param(materials, key, &layout, &name, value).is_ok() {
                *is_dirty = true;
            }
        });
    }
}
