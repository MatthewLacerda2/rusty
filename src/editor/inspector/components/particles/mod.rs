//! src/editor/inspector/components/particles/ — the Particle System inspector card.
//!
//! Edits every serde-persisted field of `ParticleEmitterComponent`: emission mode,
//! blend, collision response, the rate/burst/cap counts, the shape and per-particle
//! motion, the start ranges (`start.rs`), the over-life curves and gradient and
//! the sub-emitters (`over_life.rs`), how the particles are drawn (`render.rs`),
//! restitution, and the deterministic seed. The live particle count (transient runtime) is shown
//! read-only so an author can see the emitter working in Play.
//!
//! A THIN client (#287): each widget reads its field from a snapshot and routes
//! the write through a shared `authoring::particles::*` op. The `Particles.*` Lua
//! surface's `SetActive` / `SetRate` call the same ops — one shared write.

mod over_life;
mod render;
mod start;
mod widgets;

use egui_phosphor::regular as icon;
use widgets::{clamped, combo, drag_u32};

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::particles as particle_ops;
use crate::scene::{CollisionResponse, EmitMode, ParticleEmitterComponent};

type Cx<'w> = (&'w mut crate::ecs::World, u32); // world + entity id, as one param
/// Routes a field write through the shared particle-authoring op; a no-op absent.
fn apply(cx: &mut Cx<'_>, f: impl FnOnce(&mut ParticleEmitterComponent)) {
    if let Some(mut c) = cx.0.particles_mut(cx.1) {
        f(&mut c);
    }
}
/// 3F-particles. Particle System component card.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(p) = world.particles(id).map(|p| p.clone()) else {
        return;
    };
    let mut cx: Cx<'_> = (world, id);
    let (mut remove, mut changed) = (false, false);
    component_card(
        ui,
        icon::SPARKLE,
        "Particle System",
        Some(&mut remove),
        |ui| {
            let mut active = p.active;
            if ui.checkbox(&mut active, "Active").changed() {
                apply(&mut cx, |c| particle_ops::set_active(c, active));
                changed = true;
            }
            ui.label(format!("Live particles: {}", p.live_count()));
            ui.separator();
            changed |= draw_emission(ui, &mut cx, &p);
            ui.separator();
            changed |= render::draw(ui, &mut cx, &p);
            ui.separator();
            changed |= start::draw_shape(ui, &mut cx, &p);
            ui.separator();
            changed |= start::draw_motion(ui, &mut cx, &p);
            ui.separator();
            changed |= start::draw_start(ui, &mut cx, &p);
            ui.separator();
            changed |= over_life::draw(ui, &mut cx, &p);
            ui.separator();
            changed |= over_life::draw_sub_emitters(ui, &mut cx, &p, id);
            ui.separator();
            changed |= draw_collision(ui, &mut cx, &p);
            ui.separator();
            changed |= draw_determinism(ui, &mut cx, &p);
        },
    );
    if remove {
        cx.0.set_particles(id, None);
        *is_dirty = true;
    } else if changed {
        *is_dirty = true;
    }
}

/// Emission controls: mode, looping, rate, burst count and particle cap.
fn draw_emission(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
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
        apply(cx, |c| particle_ops::set_emit_mode(c, mode));
        changed = true;
    }
    let mut looping = p.looping;
    if ui.checkbox(&mut looping, "Looping").changed() {
        apply(cx, |c| particle_ops::set_looping(c, looping));
        changed = true;
    }
    let mut rate = p.rate;
    if clamped(ui, "Rate (per sec):", &mut rate, 0.0..=1000.0) {
        apply(cx, |c| particle_ops::set_rate(c, rate));
        changed = true;
    }
    let mut burst = p.burst_count;
    if drag_u32(ui, "Burst Count:", &mut burst, 1..=100_000) {
        apply(cx, |c| particle_ops::set_burst_count(c, burst));
        changed = true;
    }
    let mut cap = p.max_particles;
    if drag_u32(ui, "Max Particles:", &mut cap, 1..=100_000) {
        apply(cx, |c| particle_ops::set_max_particles(c, cap));
        changed = true;
    }
    changed
}

/// Collision response and bounciness controls.
fn draw_collision(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
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
        apply(cx, |c| particle_ops::set_collision(c, collision));
        changed = true;
    }
    let mut bounciness = p.bounciness;
    if clamped(ui, "Bounciness:", &mut bounciness, 0.0..=1.0) {
        apply(cx, |c| particle_ops::set_bounciness(c, bounciness));
        changed = true;
    }
    changed
}

/// The deterministic seed control.
fn draw_determinism(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
    let mut seed = p.seed;
    let changed = ui
        .horizontal(|ui| {
            ui.label("Seed:");
            ui.add(egui::DragValue::new(&mut seed).speed(1.0)).changed()
        })
        .inner;
    if changed {
        apply(cx, |c| particle_ops::set_seed(c, seed));
    }
    changed
}
