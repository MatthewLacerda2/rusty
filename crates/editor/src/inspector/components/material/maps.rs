//! The Material card's optional maps: a "Use X Map" checkbox per map slot, each
//! revealing its path field, every edit through the shared op.

use std::collections::BTreeMap;

use rusty_asset::MapRef;
use rusty_edit::authoring::material as mat_ops;
use rusty_scene::MaterialAsset;

/// The map rows, in the Inspector's order: glTF's maps, then the occlusion and
/// specular-level maps and the height map; `OcclusionStrength` sits
/// in the surface fields, `HeightScale` and its step counts in the coordinate fields.
pub(super) fn draw(ui: &mut egui::Ui, materials: &mut BTreeMap<String, MaterialAsset>, key: &str) {
    for (label, kind) in [
        ("Use Metallic Map", MapKind::Metallic),
        ("Use Roughness Map", MapKind::Roughness),
        ("Use Normal Map", MapKind::Normal),
        ("Use Emissive Map", MapKind::Emissive),
        ("Use Occlusion Map", MapKind::Occlusion),
        ("Use Specular Map", MapKind::Specular),
        ("Use Height Map", MapKind::Height),
    ] {
        optional_map(ui, label, materials, key, kind);
    }
}

#[derive(Clone, Copy)]
enum MapKind {
    Metallic,
    Roughness,
    Normal,
    Emissive,
    Occlusion,
    Specular,
    Height,
}

impl MapKind {
    fn get(self, m: &MaterialAsset) -> &Option<MapRef> {
        match self {
            MapKind::Metallic => &m.metallic_map,
            MapKind::Roughness => &m.roughness_map,
            MapKind::Normal => &m.normal_map,
            MapKind::Emissive => &m.emissive_map,
            MapKind::Occlusion => &m.occlusion_map,
            MapKind::Specular => &m.specular_map,
            MapKind::Height => &m.height_map,
        }
    }

    fn set(self, materials: &mut BTreeMap<String, MaterialAsset>, key: &str, path: Option<String>) {
        match self {
            MapKind::Metallic => mat_ops::set_metallic_map(materials, key, path),
            MapKind::Roughness => mat_ops::set_roughness_map(materials, key, path),
            MapKind::Normal => mat_ops::set_normal_map(materials, key, path),
            MapKind::Emissive => mat_ops::set_emissive_map(materials, key, path),
            MapKind::Occlusion => mat_ops::set_occlusion_map(materials, key, path),
            MapKind::Specular => mat_ops::set_specular_map(materials, key, path),
            MapKind::Height => mat_ops::set_height_map(materials, key, path),
        }
    }
}

/// A "Use X Map" checkbox that, when ticked, reveals an editable map-path field. The
/// occlusion, specular and height setters drop an empty path, so the tick is kept in
/// egui's memory until one is typed.
fn optional_map(
    ui: &mut egui::Ui,
    label: &str,
    materials: &mut BTreeMap<String, MaterialAsset>,
    key: &str,
    kind: MapKind,
) {
    let ticked = ui.id().with(("optional_map", key, label));
    let pending = ui.data(|d| d.get_temp::<bool>(ticked)).unwrap_or(false);
    let current = kind.get(&materials[key]).clone();
    let mut enabled = current.is_some() || pending;
    if ui.checkbox(&mut enabled, label).changed() {
        ui.data_mut(|d| d.insert_temp(ticked, enabled));
        kind.set(materials, key, enabled.then(String::new));
    }
    if !enabled {
        return;
    }
    let mut path = kind
        .get(&materials[key])
        .as_ref()
        .map_or_else(String::new, MapRef::to_string);
    ui.horizontal(|ui| {
        ui.label("  Path:");
        if ui.text_edit_singleline(&mut path).changed() {
            kind.set(materials, key, Some(path));
        }
    });
}
