//! A THIN client: widgets read a snapshot and route every write through the
//! shared `authoring::nav::offmesh_link` ops the `OffMeshLink.*` Lua setters call. A
//! link is a navmesh input: the editor's per-frame nav sync picks an edit
//! up (links only: no spans change).

use crate::inspector::components::card::remove_component;
use crate::widgets::DragRange;
use egui_phosphor::regular as icon;
use rusty_edit::authoring::ComponentKind;

use super::physics::vec3_row;
use crate::inspector::components::card::component_card;
use rusty_components::OffMeshLinkComponent as Link;
use rusty_edit::authoring::nav::offmesh_link as link_ops;

/// Route one edit through the shared ops.
fn write(world: &mut rusty_ecs::World, id: u32, op: impl FnOnce(&mut Link)) {
    if let Some(mut l) = world.offmesh_link_mut(id) {
        op(&mut l);
    }
}

/// The cost override and the area rows.
fn draw_cost_and_area(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, l: &Link) {
    let mut cost = l.cost_override;
    let edited = ui
        .horizontal(|ui| {
            ui.label("Cost Override:");
            ui.add(egui::DragValue::new(&mut cost).speed(0.1)).changed()
        })
        .inner;
    if edited {
        write(world, id, |l| link_ops::set_cost_override(l, cost));
    }
    ui.label("Negative cost: the link's length at its area's cost");
    let mut area = l.area;
    let edited = ui
        .horizontal(|ui| {
            ui.label("Area:");
            let max = (rusty_navigation::MAX_AREAS - 1) as u8;
            ui.add(egui::DragValue::new(&mut area).ranged(0..=max))
                .changed()
        })
        .inner;
    if edited {
        write(world, id, |l| link_ops::set_area(l, i64::from(area)));
    }
}

pub fn draw_offmesh_link(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32) {
    let Some(l) = world.offmesh_link(id).map(|l| l.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(
        ui,
        icon::LADDER_SIMPLE,
        "Off-Mesh Link",
        Some(&mut remove),
        |ui| {
            let mut active = l.active;
            if ui.checkbox(&mut active, "Active").changed() {
                write(world, id, |l| link_ops::set_active(l, active));
            }
            if let Some(v) = vec3_row(ui, "Start:", l.start) {
                write(world, id, |l| link_ops::set_start(l, v));
            }
            if let Some(v) = vec3_row(ui, "End:", l.end) {
                write(world, id, |l| link_ops::set_end(l, v));
            }
            let mut both = l.bidirectional;
            if ui.checkbox(&mut both, "Bidirectional").changed() {
                write(world, id, |l| link_ops::set_bidirectional(l, both));
            }
            draw_cost_and_area(ui, world, id, &l);
        },
    );
    if remove {
        remove_component(world, id, ComponentKind::OffMeshLink);
    }
}

impl crate::inspector::components::add::AddEntry for rusty_components::OffMeshLinkComponent {
    const CATEGORY: crate::inspector::components::add::Category =
        crate::inspector::components::add::Category::Navigation;
    const LABEL: &'static str = "Off-Mesh Link";
    const ICON: &'static str = egui_phosphor::regular::LADDER_SIMPLE;
}
