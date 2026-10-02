//! src/editor/inspector/assets/nav_areas.rs — the scene's navigation area table
//! (#460), inside the Navmesh section.
//!
//! Each row shows an area's id, name and cost; a cost edit and "Add Area" write
//! through the same `NavMeshSettings::set_area_cost` / `define_area` that
//! `Navigation.SetAreaCost` / `DefineArea` call, then hand the new costs to the
//! live graph. Costs are read at search time, so nothing is rebaked.

use crate::navigation::{NavigationGraph, MIN_AREA_COST};
use crate::scene::Scene;

/// Draw the area table; edits apply to `scene` and `nav` at once.
pub fn draw(ui: &mut egui::Ui, scene: &mut Scene, nav: &mut NavigationGraph) {
    ui.add_space(4.0);
    ui.label("Areas (cost multiplies path length; at least 1)");
    let s = &mut scene.nav_settings;
    let mut changed = false;
    for (i, area) in s.areas.clone().iter().enumerate() {
        let mut cost = area.cost;
        ui.horizontal(|ui| {
            ui.label(format!("{i}: {}", area.name));
            let drag = egui::DragValue::new(&mut cost).speed(0.1);
            if ui.add(drag.clamp_range(MIN_AREA_COST..=1000.0)).changed() {
                changed |= s.set_area_cost(&area.name, cost).is_some();
            }
        });
    }
    let name_id = ui.id().with("new_nav_area");
    let mut name: String = ui.data_mut(|d| d.get_temp(name_id).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut name);
        if ui.button("Add Area").clicked() && s.define_area(&name, 1.0).is_some() {
            changed = true;
            name.clear();
        }
    });
    ui.data_mut(|d| d.insert_temp(name_id, name));
    if changed {
        nav.set_area_costs(&scene.nav_settings.areas);
    }
}
