use std::collections::BTreeMap;

use rusty_asset::MapRef;
use rusty_edit::authoring::material as mat_ops;
use rusty_scene::MaterialAsset;

/// The runtime-param rows for material `key`; nothing when its shader has none (or
/// it names no baked shader).
pub(super) fn draw_shader_params(
    ui: &mut egui::Ui,
    materials: &mut BTreeMap<String, MaterialAsset>,
    key: &str,
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
            if changed {
                // A value the layout refuses writes nothing, so the material stays as it was.
                let _ = mat_ops::set_shader_param(materials, key, &layout, &name, value);
            }
        });
    }
}

/// A path field per extra shader texture slot, shown once the material names
/// a shader; empty clears the slot back to white.
pub(super) fn draw_shader_textures(
    ui: &mut egui::Ui,
    materials: &mut BTreeMap<String, MaterialAsset>,
    key: &str,
) {
    if materials[key].shader.is_none() {
        return;
    }
    for slot in rusty_shadergen::textures::SLOTS {
        let mut path = materials[key]
            .shader_textures
            .get(slot.name)
            .map(MapRef::to_string)
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(format!("{} texture:", slot.name));
            if ui.text_edit_singleline(&mut path).changed()
                && mat_ops::set_shader_texture(materials, key, slot.name, Some(path)).is_ok()
            {
            }
        })
        .response
        .on_hover_text("The texture this shader slot samples; empty = white");
    }
}
