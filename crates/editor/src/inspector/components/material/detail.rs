//! The Material card's detail layer, HDRP Lit's *Detail Map* section: the
//! map's path, then its mask's and its
//! table-driven rows (`DETAIL_FIELDS`), which show once a map is set.
//! The path is a plain field, with no "Use" checkbox: an empty path is no map, the
//! rule `Material.SetDetailMap("")` shares (`authoring::material::set_detail_map`),
//! so there is no empty-but-enabled state for a checkbox to hold.

use std::collections::BTreeMap;

use rusty_asset::MapRef;
use rusty_edit::authoring::material::{self as mat_ops, DETAIL_FIELDS};
use rusty_scene::MaterialAsset;

/// Draw material `key`'s detail section.
pub(super) fn draw(ui: &mut egui::Ui, materials: &mut BTreeMap<String, MaterialAsset>, key: &str) {
    let layer = materials[key].detail.as_ref();
    let mut path = layer
        .and_then(|d| d.map.as_ref())
        .map_or_else(String::new, MapRef::to_string);
    // A mask stays editable once the map is cleared, so a saved one can be cleared.
    let show_mask = layer.is_some_and(|d| d.map.is_some() || d.mask.is_some());
    let mut mask = layer
        .and_then(|d| d.mask.as_ref())
        .map_or_else(String::new, MapRef::to_string);
    ui.horizontal(|ui| {
        ui.label("Detail Map:");
        if ui.text_edit_singleline(&mut path).changed() {
            mat_ops::set_detail_map(materials, key, Some(path));
        }
    });
    if show_mask {
        ui.horizontal(|ui| {
            ui.label("Detail Mask:").on_hover_text(
                "Red blends the detail back toward the surface per texel: white full, black none. Empty applies it everywhere.",
            );
            if ui.text_edit_singleline(&mut mask).changed() {
                mat_ops::set_detail_mask(materials, key, Some(mask));
            }
        });
    }
    super::surface::draw_fields(ui, DETAIL_FIELDS, materials, key);
}
