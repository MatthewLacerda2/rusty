use crate::EditorUi;
use egui_phosphor::regular as icon;
use rusty_asset::sidecar::{
    self, Compression, FilterMode, TextureSettings, TextureType, WrapMode, MAX_SIZES,
};
use rusty_scene::{MaterialComponent, Scene};
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

/// Texture import settings: wrap mode, filter mode, mipmap generation,
/// texture type, compression and max size, kept in the image's `.meta`
fn draw_import_settings(ui: &mut egui::Ui, path: &str) {
    ui.heading("Texture Import Settings");
    ui.add_space(5.0);

    let source = Path::new(path);
    let stored = sidecar::texture(source);
    let mut s = stored;
    let names = |m: &WrapMode| m.name().to_owned();
    combo(
        ui,
        ("Wrap Mode:", "ImgWrapMode"),
        &mut s.wrap_mode,
        WrapMode::ALL,
        names,
    );
    let names = |m: &FilterMode| m.name().to_owned();
    combo(
        ui,
        ("Filter Mode:", "ImgFilterMode"),
        &mut s.filter_mode,
        FilterMode::ALL,
        names,
    );
    ui.checkbox(&mut s.generate_mipmaps, "Generate Mipmaps");
    draw_compression(ui, &mut s);
    if s.max_size != stored.max_size {
        s = s.chosen(s.max_size);
    }
    draw_size(ui, source, &s);
    if s != stored {
        let _ = sidecar::set_texture(source, s);
    }
}

/// The settings that decide its GPU format: Texture Type, sRGB (for a
/// `Default` texture, as Unity shows it), Max Size and Compression.
fn draw_compression(ui: &mut egui::Ui, s: &mut TextureSettings) {
    let names = |t: &TextureType| t.name().to_owned();
    let all = TextureType::ALL;
    combo(
        ui,
        ("Texture Type:", "ImgTextureType"),
        &mut s.texture_type,
        all,
        names,
    );
    if s.texture_type == TextureType::Default {
        // Unity shows *sRGB (Color Texture)* for a Default texture only.
        ui.checkbox(&mut s.srgb, "sRGB (Color Texture)");
    }
    draw_max_size(ui, s);
    let names = |c: &Compression| c.name().to_owned();
    combo(
        ui,
        ("Compression:", "ImgCompression"),
        &mut s.compression,
        Compression::ALL,
        names,
    );
}

fn draw_max_size(ui: &mut egui::Ui, s: &mut TextureSettings) {
    ui.horizontal(|ui| {
        ui.label("Max Size:");
        egui::ComboBox::from_id_salt("ImgMaxSize")
            .selected_text(s.max_size.to_string())
            .show_ui(ui, |ui| {
                for size in MAX_SIZES {
                    ui.selectable_value(&mut s.max_size, size, size.to_string());
                }
            });
        let muted = crate::theme::from_ui(ui).text_secondary;
        if !s.max_size_chosen {
            ui.colored_label(muted, "follows bakes");
            return;
        }
        ui.colored_label(muted, "chosen");
        let back = ui.small_button("Follow bakes");
        if back
            .on_hover_text("Hand Max Size back to the engine: a bake sets it to fit")
            .clicked()
        {
            s.max_size_chosen = false;
        }
    });
}

/// The file's size and, when its Max Size shrinks it, the size it is imported at,
/// as Unity's inspector shows the original beside the imported one.
fn draw_size(ui: &mut egui::Ui, source: &Path, s: &TextureSettings) {
    let Some(((w, h), (iw, ih))) = rusty_asset::texture_import::imported_size(source, s) else {
        return;
    };
    match (w, h) == (iw, ih) {
        true => ui.label(format!("Size: {w}x{h}")),
        false => ui.label(format!("Size: {w}x{h}, imported at {iw}x{ih} (Max Size)")),
    };
}

/// One labelled drop-down: `value` picked from `options`, each shown by `name`.
fn combo<T: Copy + PartialEq, const N: usize>(
    ui: &mut egui::Ui,
    (label, id): (&str, &str),
    value: &mut T,
    options: [T; N],
    name: impl Fn(&T) -> String,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(id)
            .selected_text(name(value))
            .show_ui(ui, |ui| {
                for option in options {
                    ui.selectable_value(value, option, name(&option));
                }
            });
    });
}

/// Entity picker that applies this texture to the chosen scene entity.
fn draw_apply_to_entity(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene, path: &str) {
    ui.heading("Apply to Scene Entity");
    ui.add_space(5.0);

    if scene.is_empty() {
        ui.colored_label(
            crate::theme::from_ui(ui).text_secondary,
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
