//! The RectTransform inspector card (#417): anchors, pivot, anchored position and
//! size delta. Every write routes through `scene::authoring::rect_transform`. The
//! Transform card above it keeps rotation and scale; its position is ignored for a
//! rect-laid-out entity. The *World Anchor* section (#429) makes the element a
//! marker pinned to a world point. The anchor-preset picker (#423) sits on top;
//! an element its parent's layout group places is shown read-only, as in Unity.

use egui_phosphor::regular as icon;

use super::{anchor_presets, entity_row, vec2_row};
use crate::components::{RectTransformComponent, WorldAnchor};
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::rect_transform as rect_ops;
use crate::ui::layout::{driven_by_group, rect_of};

type Setter = fn(&mut RectTransformComponent, glam::Vec2);

/// RectTransform card. A THIN client (#287): widgets read a snapshot and route
/// each write through a shared op; remove detaches the component. `screen` is the
/// game view's pixel size, which a preset needs to keep the rect in place.
pub fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    screen: glam::Vec2,
    is_dirty: &mut bool,
) {
    let Some(r) = world.rect_transform(id).map(|r| r.clone()) else {
        return;
    };
    let driven = driven_by_group(world, id);
    let rows: [(&str, glam::Vec2, f32, Setter); 5] = [
        ("Anchor Min:", r.anchor_min, 0.01, rect_ops::set_anchor_min),
        ("Anchor Max:", r.anchor_max, 0.01, rect_ops::set_anchor_max),
        ("Pivot:", r.pivot, 0.01, rect_ops::set_pivot),
        (
            "Position:",
            r.anchored_position,
            1.0,
            rect_ops::set_anchored_position,
        ),
        ("Size Delta:", r.size_delta, 1.0, rect_ops::set_size_delta),
    ];
    let mut remove = false;
    let mut changed = false;
    component_card(
        ui,
        icon::FRAME_CORNERS,
        "Rect Transform",
        Some(&mut remove),
        |ui| {
            if driven {
                ui.label("Driven by the parent's Layout Group.");
            }
            ui.add_enabled_ui(!driven, |ui| {
                if let Some(preset) = anchor_presets::picker(ui, &r) {
                    let parent = world.parent_id(id).and_then(|p| rect_of(world, p, screen));
                    let size = parent.map_or(glam::Vec2::ZERO, |p| p.rect.1);
                    if let Some(mut c) = world.rect_transform_mut(id) {
                        rect_ops::apply_anchor_preset(&mut c, preset, size);
                    }
                    changed = true;
                }
                for (label, value, speed, set) in rows {
                    if let Some(v) = vec2_row(ui, label, value, speed) {
                        if let Some(mut c) = world.rect_transform_mut(id) {
                            set(&mut c, v);
                        }
                        changed = true;
                    }
                }
            });
            ui.separator();
            if let Some(edit) = draw_world_anchor(ui, r.world_anchor.clone()) {
                if let Some(mut c) = world.rect_transform_mut(id) {
                    write_anchor(&mut c, edit);
                }
                changed = true;
            }
        },
    );
    if remove {
        world.set_rect_transform(id, None);
    }
    *is_dirty |= remove || changed;
}

/// The world anchor section over a snapshot. Returns the edited anchor (`None`
/// inside: not a marker) when anything changed.
fn draw_world_anchor(
    ui: &mut egui::Ui,
    anchor: Option<WorldAnchor>,
) -> Option<Option<WorldAnchor>> {
    let mut on = anchor.is_some();
    let mut a = anchor.clone().unwrap_or_default();
    let mut changed = ui.checkbox(&mut on, "World Anchor (marker)").changed();
    if on {
        changed |= entity_row(ui, "Target:", "world point", &mut a.target);
        let mut o = a.offset.to_array();
        changed |= ui
            .horizontal(|ui| {
                ui.label("Offset:");
                let mut any = false;
                for (c, prefix) in o.iter_mut().zip(["x ", "y ", "z "]) {
                    any |= ui
                        .add(egui::DragValue::new(c).speed(0.05).prefix(prefix))
                        .changed();
                }
                any
            })
            .inner;
        a.offset = glam::Vec3::from_array(o);
        changed |= ui
            .checkbox(&mut a.clamp_to_screen_edge, "Clamp to screen edge")
            .changed();
        changed |= ui
            .horizontal(|ui| {
                ui.label("Edge Padding:");
                ui.add(egui::DragValue::new(&mut a.edge_padding)).changed()
            })
            .inner;
        changed |= ui
            .checkbox(&mut a.rotate_toward_target, "Rotate toward target")
            .changed();
        changed |= ui
            .checkbox(&mut a.hide_when_behind, "Hide when behind")
            .changed();
    }
    changed.then(|| on.then_some(a))
}

/// Write an edited world anchor back through the shared ops.
fn write_anchor(r: &mut RectTransformComponent, edit: Option<WorldAnchor>) {
    let Some(a) = edit else {
        rect_ops::clear_world_anchor(r);
        return;
    };
    rect_ops::set_world_anchor(r, a.target, a.offset);
    rect_ops::set_anchor_clamp(r, a.clamp_to_screen_edge, a.edge_padding);
    rect_ops::set_anchor_rotate(r, a.rotate_toward_target);
    rect_ops::set_anchor_hide_when_behind(r, a.hide_when_behind);
}
