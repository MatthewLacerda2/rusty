//! The RectTransform inspector card (#417): anchors, pivot, anchored position and
//! size delta. Every write routes through `scene::authoring::rect_transform`. The
//! Transform card above it keeps rotation and scale; its position is ignored for a
//! rect-laid-out entity.

use egui_phosphor::regular as icon;

use super::vec2_row;
use crate::components::RectTransformComponent;
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::rect_transform as rect_ops;

type Setter = fn(&mut RectTransformComponent, glam::Vec2);

/// RectTransform card. A THIN client (#287): widgets read a snapshot and route
/// each write through a shared op; remove detaches the component.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(r) = world.rect_transform(id).map(|r| r.clone()) else {
        return;
    };
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
            for (label, value, speed, set) in rows {
                if let Some(v) = vec2_row(ui, label, value, speed) {
                    if let Some(mut c) = world.rect_transform_mut(id) {
                        set(&mut c, v);
                    }
                    changed = true;
                }
            }
        },
    );
    if remove {
        world.set_rect_transform(id, None);
    }
    *is_dirty |= remove || changed;
}
