mod over_life;
mod render;
mod start;
pub(in crate::inspector::components) mod widgets;

use crate::inspector::components::card::remove_component;
use egui_phosphor::regular as icon;
use rusty_edit::authoring::ComponentKind;
use widgets::{clamped, combo, drag_u32};

use crate::inspector::components::card::component_card;
use rusty_components::particle::GPU_MAX_PARTICLES;
use rusty_components::{ParticleSimulation, SimulationSpace};
use rusty_edit::authoring::particles as particle_ops;
use rusty_scene::{CollisionResponse, EmitMode, ParticleEmitterComponent};

/// The Rate (per second) and Max Particles ranges: gameplay counts on `Cpu`; on `Gpu`,
/// its fixed cap, and a rate that fills it in a second.
fn emission_ranges(
    simulation: ParticleSimulation,
) -> (std::ops::RangeInclusive<f32>, std::ops::RangeInclusive<u32>) {
    match simulation {
        ParticleSimulation::Cpu => (0.0..=1000.0, 1..=100_000),
        ParticleSimulation::Gpu => (0.0..=GPU_MAX_PARTICLES as f32, 1..=GPU_MAX_PARTICLES),
    }
}

type Cx<'w> = (&'w mut rusty_ecs::World, u32);
/// Routes a field write through the shared particle-authoring op; a no-op absent.
fn apply(cx: &mut Cx<'_>, f: impl FnOnce(&mut ParticleEmitterComponent)) {
    if let Some(mut c) = cx.0.particles_mut(cx.1) {
        f(&mut c);
    }
}
/// 3F-particles. Particle System component card.
pub fn draw(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, is_dirty: &mut bool) {
    // The configuration only: the live particles are never copied (a `Gpu`
    // emitter keeps a birth per live particle).
    let Some((p, live)) = world.particles(id).map(|p| (p.config(), p.live_count())) else {
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
            ui.label(format!("Live particles: {live}"));
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
        remove_component(cx.0, id, ComponentKind::Particles, is_dirty);
    } else if changed {
        *is_dirty = true;
    }
}

/// Emission controls: mode, looping, simulation space and target (with what a
/// `Gpu` emitter does not honour), rate, burst count and particle cap.
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
    let mut space = p.simulation_space;
    let spaces = SimulationSpace::ALL.map(|s| (s, s.name()));
    if combo(ui, "Simulation Space", &mut space, &spaces) {
        apply(cx, |c| particle_ops::set_simulation_space(c, space));
        changed = true;
    }
    let mut target = p.simulation;
    let targets = ParticleSimulation::ALL.map(|s| (s, s.name()));
    if combo(ui, "Simulation", &mut target, &targets) {
        apply(cx, |c| particle_ops::set_simulation(c, target));
        changed = true;
    }
    for line in particle_ops::gpu_warnings(p) {
        ui.colored_label(ui.visuals().warn_fg_color, line);
    }
    draw_counts(ui, cx, p) || changed
}

/// The rate, burst count and particle cap, ranged by the simulation target.
fn draw_counts(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
    let mut changed = false;
    let (rates, caps) = emission_ranges(p.simulation);
    let mut rate = p.rate;
    if clamped(ui, "Rate (per sec):", &mut rate, rates) {
        apply(cx, |c| particle_ops::set_rate(c, rate));
        changed = true;
    }
    let mut burst = p.burst_count;
    if drag_u32(ui, "Burst Count:", &mut burst, 1..=100_000) {
        apply(cx, |c| particle_ops::set_burst_count(c, burst));
        changed = true;
    }
    let mut cap = p.max_particles;
    if drag_u32(ui, "Max Particles:", &mut cap, caps) {
        apply(cx, |c| particle_ops::set_max_particles(c, cap));
        changed = true;
    }
    changed
}

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

impl crate::inspector::components::add::AddEntry for rusty_components::ParticleEmitterComponent {
    const CATEGORY: crate::inspector::components::add::Category =
        crate::inspector::components::add::Category::Effects;
    const LABEL: &'static str = "Particle System";
    const ICON: &'static str = egui_phosphor::regular::SPARKLE;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gpu_emitters_fields_reach_its_cap_and_a_cpu_ones_stay_at_gameplay_counts() {
        let (rates, caps) = emission_ranges(ParticleSimulation::Gpu);
        assert_eq!(*caps.end(), GPU_MAX_PARTICLES);
        assert!(
            *rates.end() >= GPU_MAX_PARTICLES as f32,
            "a rate that fills the cap in 1 s"
        );
        let (rates, caps) = emission_ranges(ParticleSimulation::Cpu);
        assert_eq!((*rates.end(), *caps.end()), (1000.0, 100_000));
    }
}
