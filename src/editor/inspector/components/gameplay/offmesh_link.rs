//! src/editor/inspector/components/gameplay/offmesh_link.rs — the Off-Mesh Link
//! card (#462).
//!
//! A THIN client (#287): widgets read a snapshot and route every write through the
//! shared `authoring::offmesh_link` ops the `OffMeshLink.*` Lua setters call. A
//! link is a navmesh input, so every edit asks for a nav rebake (links only: no
//! spans change).

use egui_phosphor::regular as icon;

use super::physics::vec3_row;
use crate::components::OffMeshLinkComponent as Link;
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::offmesh_link as link_ops;

/// Route one edit through the shared ops, mark the scene dirty, ask for a rebake.
fn write(
    world: &mut crate::ecs::World,
    id: u32,
    flags: (&mut bool, &mut bool),
    op: impl FnOnce(&mut Link),
) {
    if let Some(mut l) = world.offmesh_link_mut(id) {
        op(&mut l);
    }
    *flags.0 = true;
    *flags.1 = true;
}

/// The cost override and the area (#460) rows.
fn draw_cost_and_area(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    l: &Link,
    (d, b): (&mut bool, &mut bool),
) {
    let mut cost = l.cost_override;
    let edited = ui
        .horizontal(|ui| {
            ui.label("Cost Override:");
            ui.add(egui::DragValue::new(&mut cost).speed(0.1)).changed()
        })
        .inner;
    if edited {
        write(world, id, (&mut *d, &mut *b), |l| {
            link_ops::set_cost_override(l, cost)
        });
    }
    ui.label("Negative cost: the link's length at its area's cost");
    let mut area = l.area;
    let edited = ui
        .horizontal(|ui| {
            ui.label("Area:");
            let max = (crate::navigation::MAX_AREAS - 1) as u8;
            ui.add(egui::DragValue::new(&mut area).clamp_range(0..=max))
                .changed()
        })
        .inner;
    if edited {
        write(world, id, (d, b), |l| {
            link_ops::set_area(l, i64::from(area))
        });
    }
}

/// 3EM. OffMeshLink Component
pub fn draw_offmesh_link(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
    pending_nav_bake: &mut bool,
) {
    let Some(l) = world.offmesh_link(id).map(|l| l.clone()) else {
        return;
    };
    let (d, b) = (is_dirty, pending_nav_bake);
    let mut remove = false;
    component_card(
        ui,
        icon::LADDER_SIMPLE,
        "Off-Mesh Link",
        Some(&mut remove),
        |ui| {
            let mut active = l.active;
            if ui.checkbox(&mut active, "Active").changed() {
                write(world, id, (&mut *d, &mut *b), |l| {
                    link_ops::set_active(l, active)
                });
            }
            if let Some(v) = vec3_row(ui, "Start:", l.start) {
                write(world, id, (&mut *d, &mut *b), |l| link_ops::set_start(l, v));
            }
            if let Some(v) = vec3_row(ui, "End:", l.end) {
                write(world, id, (&mut *d, &mut *b), |l| link_ops::set_end(l, v));
            }
            let mut both = l.bidirectional;
            if ui.checkbox(&mut both, "Bidirectional").changed() {
                write(world, id, (&mut *d, &mut *b), |l| {
                    link_ops::set_bidirectional(l, both)
                });
            }
            draw_cost_and_area(ui, world, id, &l, (&mut *d, &mut *b));
        },
    );
    if remove {
        world.set_offmesh_link(id, None);
        *d = true;
        *b = true;
    }
}
