//! The Material card's detail layer (#1997), HDRP Lit's *Detail Map* section: the
//! map's path, then its table-driven rows (`DETAIL_FIELDS`), which show once a
//! path is set.
//!
//! The path is a plain field, with no "Use" checkbox: an empty path is no map, the
//! rule `Material.SetDetailMap("")` shares (`authoring::material::set_detail_map`),
//! so there is no empty-but-enabled state for a checkbox to hold.

use std::collections::BTreeMap;

use rusty_asset::MapRef;
use rusty_scene::authoring::material::{self as mat_ops, DETAIL_FIELDS};
use rusty_scene::MaterialAsset;

/// Draw material `key`'s detail section.
pub(super) fn draw(
    ui: &mut egui::Ui,
    materials: &mut BTreeMap<String, MaterialAsset>,
    key: &str,
    is_dirty: &mut bool,
) {
    let layer = materials[key].detail.as_ref();
    let mut path = layer
        .and_then(|d| d.map.as_ref())
        .map_or_else(String::new, MapRef::to_string);
    ui.horizontal(|ui| {
        ui.label("Detail Map:");
        if ui.text_edit_singleline(&mut path).changed() {
            mat_ops::set_detail_map(materials, key, Some(path));
            *is_dirty = true;
        }
    });
    super::surface::draw_fields(ui, DETAIL_FIELDS, materials, key, is_dirty);
}
