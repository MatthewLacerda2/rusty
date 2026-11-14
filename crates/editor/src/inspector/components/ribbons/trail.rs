//! crates/editor/src/inspector/components/ribbons/trail.rs — the Trail card (#441).

use crate::inspector::components::card::remove_component;
use egui_phosphor::regular as icon;
use rusty_scene::authoring::ComponentKind;

use super::draw_style;
use crate::inspector::components::card::component_card;
use crate::inspector::components::particles::widgets::clamped;
use rusty_components::TrailComponent;
use rusty_scene::authoring::ribbon::trail as trail_ops;

/// Route one edit through the shared ops and mark the scene dirty.
fn write(
    world: &mut rusty_ecs::World,
    id: u32,
    dirty: &mut bool,
    op: impl FnOnce(&mut TrailComponent),
) {
    if let Some(mut t) = world.trail_mut(id) {
        op(&mut t);
    }
    *dirty = true;
}

/// 3FT. Trail Renderer card: emitting, lifetime, min vertex distance, the live
/// point count (read-only), and the shared Style section.
pub fn draw_trail(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, dirty: &mut bool) {
    let Some(t) = world.trail(id).map(|t| t.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::WIND, "Trail Renderer", Some(&mut remove), |ui| {
        let mut emitting = t.emitting;
        if ui.checkbox(&mut emitting, "Emitting").changed() {
            write(world, id, dirty, |c| trail_ops::set_emitting(c, emitting));
        }
        let mut time = t.time;
        if clamped(ui, "Time (sec):", &mut time, 0.0..=60.0) {
            write(world, id, dirty, |c| trail_ops::set_time(c, time));
        }
        let mut spacing = t.min_vertex_distance;
        if clamped(ui, "Min Vertex Distance:", &mut spacing, 0.0..=100.0) {
            write(world, id, dirty, |c| {
                trail_ops::set_min_vertex_distance(c, spacing)
            });
        }
        ui.label(format!("Recorded points: {}", t.runtime.points.len()));
        if let Some(edit) = draw_style(ui, &t.style) {
            write(world, id, dirty, |c| edit.apply(&mut c.style));
        }
    });
    if remove {
        remove_component(world, id, ComponentKind::Trail, dirty);
    }
}

impl crate::inspector::components::add::AddEntry for rusty_components::TrailComponent {
    const CATEGORY: crate::inspector::components::add::Category =
        crate::inspector::components::add::Category::Effects;
    const LABEL: &'static str = "Trail Renderer";
    const ICON: &'static str = egui_phosphor::regular::WIND;
}
