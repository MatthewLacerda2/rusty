use crate::asset::sidecar::{self, Compression, FilterMode, WrapMode, MAX_SIZES};
use crate::editor::EditorUi;
use crate::scene::{MaterialComponent, Scene};
use egui_phosphor::regular as icon;
use std::path::Path;

pub fn draw(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    ui.heading(format!("{}  Image: {}", icon::IMAGE, filename));
    ui.add_space(5.0);

    super::metadata::draw_metadata_card(ui, path, |ui| {
        ui.label("Type: Texture Asset");
    });
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);

    draw_import_settings(ui, path);
    ui.add_space(15.0);
    ui.separator();
    ui.add_space(5.0);

    draw_apply_to_entity(ui, editor, scene, path);
}

/// Texture import settings (#1323): wrap mode, filter mode, mipmap generation,
/// compression and max size (#1574), kept in the image's `.meta` sidecar. An edit goes through the setter
/// `Assets.SetTextureImportSettings` uses, so the renderer re-uploads the texture.
fn draw_import_settings(ui: &mut egui::Ui, path: &str) {
    ui.heading("Texture Import Settings");
    ui.add_space(5.0);

    let source = Path::new(path);
    let stored = sidecar::texture(source);
    let mut s = stored;
    ui.horizontal(|ui| {
        ui.label("Wrap Mode:");
        egui::ComboBox::from_id_salt("ImgWrapMode")
            .selected_text(s.wrap_mode.name())
            .show_ui(ui, |ui| {
                for mode in WrapMode::ALL {
                    ui.selectable_value(&mut s.wrap_mode, mode, mode.name());
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Filter Mode:");
        egui::ComboBox::from_id_salt("ImgFilterMode")
            .selected_text(s.filter_mode.name())
            .show_ui(ui, |ui| {
                for mode in FilterMode::ALL {
                    ui.selectable_value(&mut s.filter_mode, mode, mode.name());
                }
            });
    });
    ui.checkbox(&mut s.generate_mipmaps, "Generate Mipmaps");
    ui.horizontal(|ui| {
        ui.label("Max Size:");
        egui::ComboBox::from_id_salt("ImgMaxSize")
            .selected_text(s.max_size.to_string())
            .show_ui(ui, |ui| {
                for size in MAX_SIZES {
                    ui.selectable_value(&mut s.max_size, size, size.to_string());
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Compression:");
        egui::ComboBox::from_id_salt("ImgCompression")
            .selected_text(s.compression.name())
            .show_ui(ui, |ui| {
                for c in Compression::ALL {
                    ui.selectable_value(&mut s.compression, c, c.name());
                }
            });
    });
    if s != stored {
        let _ = sidecar::set_texture(source, s);
    }
}

/// Entity picker that applies this texture to the chosen scene entity.
fn draw_apply_to_entity(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    ui.heading("Apply to Scene Entity");
    ui.add_space(5.0);

    if scene.is_empty() {
        ui.colored_label(
            crate::editor::theme::from_ui(ui).text_secondary,
            "No entities in scene to apply texture to.",
        );
        return;
    }

    ui.label("Choose an entity to apply this texture:");
    let selected_ent_name = "Select Entity...".to_string();

    let mut clicked_entity_id = None;
    egui::ComboBox::from_id_salt("ApplyTextureToEntity")
        .selected_text(selected_ent_name)
        .show_ui(ui, |ui| {
            for id in scene.entity_ids() {
                let Some(name) = scene.world.name(id).map(|n| n.clone()) else {
                    continue;
                };
                if ui.selectable_label(false, &name).clicked() {
                    clicked_entity_id = Some(id);
                }
            }
        });

    if let Some(entity_id) = clicked_entity_id {
        apply_albedo_to_entity(scene, entity_id, path);
        editor.is_dirty = true;
    }
}

/// Attach (or update) a material on `entity_id` whose albedo is `path`, creating a
/// library material under a per-entity key if the entity has none yet. Reuses the
/// entity's existing referenced material when present so its other params survive.
fn apply_albedo_to_entity(scene: &mut Scene, entity_id: u32, path: &str) {
    if !scene.world.has_material(entity_id)
        && !scene
            .world
            .set_material(entity_id, Some(MaterialComponent::for_entity(entity_id)))
    {
        return;
    }
    let Some(key) = scene
        .world
        .material(entity_id)
        .map(|m| m.first().to_string())
    else {
        return;
    };
    let mat = scene.materials.entry(key).or_default();
    mat.base_color_map = Some(path.into());
}
