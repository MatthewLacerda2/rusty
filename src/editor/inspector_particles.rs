//! src/editor/inspector_particles.rs — the Particle System inspector card.
//!
//! Edits every serde-persisted field of `ParticleEmitterComponent`: emission mode,
//! blend, collision response, the rate/burst/cap counts, the per-particle motion
//! (lifetime/speed/direction/spread/gravity), size + colour over life, restitution,
//! and the deterministic seed. The live particle count (transient runtime) is shown
//! read-only so an author can see the emitter working in Play.

use egui_phosphor::regular as icon;

use crate::editor::inspector_card::component_card;
use crate::scene::{CollisionResponse, EmitMode, Entity, ParticleBlend};

/// 3F-particles. Particle System component card.
#[allow(clippy::too_many_lines)]
pub fn draw(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    if entity.particles.is_none() {
        return;
    }
    let mut remove = false;
    let mut changed = false;
    component_card(
        ui,
        icon::SPARKLE,
        "Particle System",
        Some(&mut remove),
        |ui| {
            let Some(p) = &mut entity.particles else {
                return;
            };

            changed |= ui.checkbox(&mut p.active, "Active").changed();
            ui.label(format!("Live particles: {}", p.live_count()));
            ui.separator();

            // Emission ---------------------------------------------------------
            changed |= combo(
                ui,
                "Emit Mode",
                &mut p.emit_mode,
                &[
                    (EmitMode::Continuous, "Continuous"),
                    (EmitMode::Burst, "Burst"),
                ],
            );
            changed |= ui.checkbox(&mut p.looping, "Looping").changed();
            changed |= clamped(ui, "Rate (per sec):", &mut p.rate, 0.0..=1000.0);
            changed |= drag_u32(ui, "Burst Count:", &mut p.burst_count, 1..=100_000);
            changed |= drag_u32(ui, "Max Particles:", &mut p.max_particles, 1..=100_000);
            ui.separator();

            // Rendering --------------------------------------------------------
            changed |= combo(
                ui,
                "Blend",
                &mut p.blend,
                &[
                    (ParticleBlend::Alpha, "Alpha"),
                    (ParticleBlend::Additive, "Additive"),
                ],
            );
            let mut tex = p.texture.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("Texture:");
                if ui.text_edit_singleline(&mut tex).changed() {
                    p.texture = if tex.is_empty() { None } else { Some(tex) };
                    changed = true;
                }
            });
            ui.separator();

            // Motion -----------------------------------------------------------
            changed |= clamped(ui, "Lifetime (sec):", &mut p.lifetime, 0.0..=60.0);
            changed |= clamped(ui, "Speed:", &mut p.speed, 0.0..=100.0);
            changed |= vec3(ui, "Direction:", &mut p.direction, 0.05);
            changed |= clamped(ui, "Spread:", &mut p.spread, 0.0..=1.0);
            changed |= vec3(ui, "Gravity:", &mut p.gravity, 0.1);
            ui.separator();

            // Size + colour ----------------------------------------------------
            changed |= clamped(ui, "Size Start:", &mut p.size_start, 0.0..=50.0);
            changed |= clamped(ui, "Size End:", &mut p.size_end, 0.0..=50.0);
            ui.horizontal(|ui| {
                ui.label("Color (RGBA):");
                if ui
                    .color_edit_button_rgba_unmultiplied(&mut p.color)
                    .changed()
                {
                    changed = true;
                }
            });
            ui.separator();

            // Collision --------------------------------------------------------
            changed |= combo(
                ui,
                "Collision",
                &mut p.collision,
                &[
                    (CollisionResponse::None, "None"),
                    (CollisionResponse::Die, "Die"),
                    (CollisionResponse::Bounce, "Bounce"),
                ],
            );
            changed |= clamped(ui, "Bounciness:", &mut p.bounciness, 0.0..=1.0);
            ui.separator();

            // Determinism ------------------------------------------------------
            ui.horizontal(|ui| {
                ui.label("Seed:");
                changed |= ui
                    .add(egui::DragValue::new(&mut p.seed).speed(1.0))
                    .changed();
            });
        },
    );
    if remove {
        entity.particles = None;
        *is_dirty = true;
    } else if changed {
        *is_dirty = true;
    }
}

/// A labelled, clamped `f32` drag row. Returns whether the value changed.
fn clamped(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(0.05).clamp_range(range))
            .changed()
    })
    .inner
}

/// A labelled, clamped `u32` drag row. Returns whether the value changed.
fn drag_u32(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(1.0).clamp_range(range))
            .changed()
    })
    .inner
}

/// A labelled x/y/z drag row over a `Vec3`. Returns whether any axis changed.
fn vec3(ui: &mut egui::Ui, label: &str, value: &mut glam::Vec3, speed: f32) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut c = ui
            .add(egui::DragValue::new(&mut value.x).speed(speed))
            .changed();
        c |= ui
            .add(egui::DragValue::new(&mut value.y).speed(speed))
            .changed();
        c |= ui
            .add(egui::DragValue::new(&mut value.z).speed(speed))
            .changed();
        c
    })
    .inner
}

/// A combo box over a small set of `Copy + PartialEq` enum variants with labels.
/// Returns whether the selection changed.
fn combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let selected = options
        .iter()
        .find(|(v, _)| *v == *value)
        .map(|(_, name)| *name)
        .unwrap_or("");
    let mut changed = false;
    egui::ComboBox::from_label(label)
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (variant, name) in options {
                if ui.selectable_label(*value == *variant, *name).clicked() {
                    *value = *variant;
                    changed = true;
                }
            }
        });
    changed
}
