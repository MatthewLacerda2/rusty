//! src/editor/inspector_particles.rs — the Particle System inspector card.
//!
//! Edits every serde-persisted field of `ParticleEmitterComponent`: emission mode,
//! blend, collision response, the rate/burst/cap counts, the per-particle motion
//! (lifetime/speed/direction/spread/gravity), size + colour over life, restitution,
//! and the deterministic seed. The live particle count (transient runtime) is shown
//! read-only so an author can see the emitter working in Play.
//!
//! A THIN client (#287): each widget reads its field from a snapshot and routes the
//! write through a shared `authoring::particles::*` op — never a mutable component
//! borrow for a field edit. The `Particles.*` Lua surface's `SetActive` / `SetRate`
//! call the same ops, so the panel and the binding share one write.

use egui_phosphor::regular as icon;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::particles as particle_ops;
use crate::scene::{CollisionResponse, EmitMode, Entity, ParticleBlend, ParticleEmitterComponent};

/// 3F-particles. Particle System component card.
pub fn draw(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let Some(p) = entity.particles.clone() else {
        return;
    };
    let mut remove = false;
    let mut changed = false;
    component_card(
        ui,
        icon::SPARKLE,
        "Particle System",
        Some(&mut remove),
        |ui| {
            let mut active = p.active;
            if ui.checkbox(&mut active, "Active").changed() {
                particle_ops::set_active(entity, active);
                changed = true;
            }
            ui.label(format!("Live particles: {}", p.live_count()));
            ui.separator();
            changed |= draw_emission(ui, entity, &p);
            ui.separator();
            changed |= draw_rendering(ui, entity, &p);
            ui.separator();
            changed |= draw_motion(ui, entity, &p);
            ui.separator();
            changed |= draw_size_color(ui, entity, &p);
            ui.separator();
            changed |= draw_collision(ui, entity, &p);
            ui.separator();
            changed |= draw_determinism(ui, entity, &p);
        },
    );
    if remove {
        entity.particles = None;
        *is_dirty = true;
    } else if changed {
        *is_dirty = true;
    }
}

/// Emission controls: mode, looping, rate, burst count and particle cap.
fn draw_emission(ui: &mut egui::Ui, entity: &mut Entity, p: &ParticleEmitterComponent) -> bool {
    let mut changed = false;
    let mut mode = p.emit_mode;
    if combo(
        ui,
        "Emit Mode",
        &mut mode,
        &[
            (EmitMode::Continuous, "Continuous"),
            (EmitMode::Burst, "Burst"),
        ],
    ) {
        particle_ops::set_emit_mode(entity, mode);
        changed = true;
    }
    let mut looping = p.looping;
    if ui.checkbox(&mut looping, "Looping").changed() {
        particle_ops::set_looping(entity, looping);
        changed = true;
    }
    let mut rate = p.rate;
    if clamped(ui, "Rate (per sec):", &mut rate, 0.0..=1000.0) {
        particle_ops::set_rate(entity, rate);
        changed = true;
    }
    let mut burst = p.burst_count;
    if drag_u32(ui, "Burst Count:", &mut burst, 1..=100_000) {
        particle_ops::set_burst_count(entity, burst);
        changed = true;
    }
    let mut cap = p.max_particles;
    if drag_u32(ui, "Max Particles:", &mut cap, 1..=100_000) {
        particle_ops::set_max_particles(entity, cap);
        changed = true;
    }
    changed
}

/// Rendering controls: blend mode and the optional texture path.
fn draw_rendering(ui: &mut egui::Ui, entity: &mut Entity, p: &ParticleEmitterComponent) -> bool {
    let mut changed = false;
    let mut blend = p.blend;
    if combo(
        ui,
        "Blend",
        &mut blend,
        &[
            (ParticleBlend::Alpha, "Alpha"),
            (ParticleBlend::Additive, "Additive"),
        ],
    ) {
        particle_ops::set_blend(entity, blend);
        changed = true;
    }
    let mut tex = p.texture.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Texture:");
        if ui.text_edit_singleline(&mut tex).changed() {
            particle_ops::set_texture(entity, tex);
            changed = true;
        }
    });
    changed
}

/// Per-particle motion: lifetime, speed, direction, spread and gravity.
fn draw_motion(ui: &mut egui::Ui, entity: &mut Entity, p: &ParticleEmitterComponent) -> bool {
    let mut changed = false;
    let mut lifetime = p.lifetime;
    if clamped(ui, "Lifetime (sec):", &mut lifetime, 0.0..=60.0) {
        particle_ops::set_lifetime(entity, lifetime);
        changed = true;
    }
    let mut speed = p.speed;
    if clamped(ui, "Speed:", &mut speed, 0.0..=100.0) {
        particle_ops::set_speed(entity, speed);
        changed = true;
    }
    let mut direction = p.direction;
    if vec3(ui, "Direction:", &mut direction, 0.05) {
        particle_ops::set_direction(entity, direction);
        changed = true;
    }
    let mut spread = p.spread;
    if clamped(ui, "Spread:", &mut spread, 0.0..=1.0) {
        particle_ops::set_spread(entity, spread);
        changed = true;
    }
    let mut gravity = p.gravity;
    if vec3(ui, "Gravity:", &mut gravity, 0.1) {
        particle_ops::set_gravity(entity, gravity);
        changed = true;
    }
    changed
}

/// Size-over-life and colour controls.
fn draw_size_color(ui: &mut egui::Ui, entity: &mut Entity, p: &ParticleEmitterComponent) -> bool {
    let mut changed = false;
    let mut size_start = p.size_start;
    if clamped(ui, "Size Start:", &mut size_start, 0.0..=50.0) {
        particle_ops::set_size_start(entity, size_start);
        changed = true;
    }
    let mut size_end = p.size_end;
    if clamped(ui, "Size End:", &mut size_end, 0.0..=50.0) {
        particle_ops::set_size_end(entity, size_end);
        changed = true;
    }
    let mut color = p.color;
    ui.horizontal(|ui| {
        ui.label("Color (RGBA):");
        if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
            particle_ops::set_color(entity, color);
            changed = true;
        }
    });
    changed
}

/// Collision response and bounciness controls.
fn draw_collision(ui: &mut egui::Ui, entity: &mut Entity, p: &ParticleEmitterComponent) -> bool {
    let mut changed = false;
    let mut collision = p.collision;
    if combo(
        ui,
        "Collision",
        &mut collision,
        &[
            (CollisionResponse::None, "None"),
            (CollisionResponse::Die, "Die"),
            (CollisionResponse::Bounce, "Bounce"),
        ],
    ) {
        particle_ops::set_collision(entity, collision);
        changed = true;
    }
    let mut bounciness = p.bounciness;
    if clamped(ui, "Bounciness:", &mut bounciness, 0.0..=1.0) {
        particle_ops::set_bounciness(entity, bounciness);
        changed = true;
    }
    changed
}

/// The deterministic seed control.
fn draw_determinism(ui: &mut egui::Ui, entity: &mut Entity, p: &ParticleEmitterComponent) -> bool {
    let mut seed = p.seed;
    let changed = ui
        .horizontal(|ui| {
            ui.label("Seed:");
            ui.add(egui::DragValue::new(&mut seed).speed(1.0)).changed()
        })
        .inner;
    if changed {
        particle_ops::set_seed(entity, seed);
    }
    changed
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
