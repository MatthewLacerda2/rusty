//! src/editor/inspector/components/particles/over_life.rs — the over-life curves,
//! the colour gradient and the sub-emitter pickers of the Particle System card.

use super::widgets::{clamped, curve_editor};
use super::{apply, Cx};
use crate::components::{ParticleEmitterComponent, SubEmitTrigger};
use crate::core::curve::{ColorKey, Gradient};
use crate::scene::authoring::particles as particle_ops;

/// Size / drag / rotation-speed curves and the colour gradient.
pub(super) fn draw(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
    let mut changed = false;
    ui.label("Over Lifetime");
    if let Some(c) = curve_editor(ui, "Size (multiplier)", &p.size_over_life) {
        apply(cx, |e| particle_ops::set_size_over_life(e, c));
        changed = true;
    }
    if let Some(g) = gradient_editor(ui, &p.color_over_life) {
        apply(cx, |e| particle_ops::set_color_over_life(e, g));
        changed = true;
    }
    if let Some(c) = curve_editor(ui, "Drag (per sec)", &p.drag) {
        apply(cx, |e| particle_ops::set_drag(e, c));
        changed = true;
    }
    if let Some(c) = curve_editor(ui, "Rotation Speed (deg/sec)", &p.rotation_speed) {
        apply(cx, |e| particle_ops::set_rotation_speed(e, c));
        changed = true;
    }
    changed
}

/// Colour keys (t + RGB, removable, addable) and the alpha curve.
fn gradient_editor(ui: &mut egui::Ui, gradient: &Gradient) -> Option<Gradient> {
    let mut g = gradient.clone();
    let mut changed = false;
    egui::CollapsingHeader::new("Color (multiplier)").show(ui, |ui| {
        let mut remove = None;
        for (i, key) in g.color_keys.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label("t");
                let t = egui::DragValue::new(&mut key.t)
                    .speed(0.01)
                    .clamp_range(0.0..=1.0);
                changed |= ui.add(t).changed();
                changed |= ui.color_edit_button_rgb(&mut key.color).changed();
                if ui.small_button("x").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            g.color_keys.remove(i);
            changed = true;
        }
        if ui.button("Add color key").clicked() {
            g.color_keys.push(ColorKey {
                t: 1.0,
                color: [1.0; 3],
            });
            changed = true;
        }
        if let Some(alpha) = curve_editor(ui, "Alpha", &g.alpha) {
            g.alpha = alpha;
            changed = true;
        }
    });
    changed.then_some(g)
}

/// One picker per trigger (any other entity with an emitter, or none), plus the
/// inherited-velocity fraction.
pub(super) fn draw_sub_emitters(
    ui: &mut egui::Ui,
    cx: &mut Cx<'_>,
    p: &ParticleEmitterComponent,
    self_id: u32,
) -> bool {
    let mut changed = false;
    ui.label("Sub-Emitters");
    let targets: Vec<(u32, String)> =
        cx.0.ids_with_particles()
            .into_iter()
            .filter(|&id| id != self_id)
            .map(|id| (id, target_label(cx.0, id)))
            .collect();
    for trigger in SubEmitTrigger::ALL {
        let current = p.sub_emitters.get(trigger);
        let mut picked = current;
        let shown = current.map_or("None".to_string(), |id| target_label(cx.0, id));
        egui::ComboBox::from_label(format!("On {}", trigger.name()))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut picked, None, "None");
                for (id, label) in &targets {
                    ui.selectable_value(&mut picked, Some(*id), label);
                }
            });
        if picked != current {
            apply(cx, |c| particle_ops::set_sub_emitter(c, trigger, picked));
            changed = true;
        }
    }
    let mut inherit = p.sub_emitters.inherit_velocity;
    if clamped(ui, "Inherit Velocity:", &mut inherit, 0.0..=1.0) {
        apply(cx, |c| particle_ops::set_inherit_velocity(c, inherit));
        changed = true;
    }
    changed
}

/// `"Name (#id)"` for an entity.
fn target_label(world: &crate::ecs::World, id: u32) -> String {
    let name = world.name(id).map_or(String::new(), |n| n.clone());
    format!("{name} (#{id})")
}
