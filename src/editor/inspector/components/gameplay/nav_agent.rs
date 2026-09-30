//! src/editor/inspector/components/gameplay/nav_agent.rs — the NavMesh Agent
//! inspector card, split from `physics` to stay under the size cap.
//!
//! A THIN client (#287): widgets read fields from a snapshot and route every write
//! through `authoring::nav_agent`, the same ops the `NavMeshAgent.*` Lua setters call.

use egui_phosphor::regular as icon;
use glam::Vec3;

use super::physics::vec3_row;
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::nav_agent as nav_ops;

/// 3EH. NavMeshAgent Component
pub fn draw_nav_agent(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
) {
    let Some(agent) = world.nav_agent(id).map(|a| a.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::PATH, "NavMesh Agent", Some(&mut remove), |ui| {
        let mut active = agent.active;
        if ui.checkbox(&mut active, "Active").changed() {
            if let Some(mut a) = world.nav_agent_mut(id) {
                nav_ops::set_active(&mut a, active);
            }
            *is_dirty = true;
        }
        let mut speed = agent.speed;
        if clamped(ui, "Speed:", &mut speed, 0.0..=100.0) {
            if let Some(mut a) = world.nav_agent_mut(id) {
                nav_ops::set_speed(&mut a, speed);
            }
            *is_dirty = true;
        }
        let mut acceleration = agent.acceleration;
        if clamped(ui, "Acceleration:", &mut acceleration, 0.0..=100.0) {
            if let Some(mut a) = world.nav_agent_mut(id) {
                nav_ops::set_acceleration(&mut a, acceleration);
            }
            *is_dirty = true;
        }
        let mut stopping_distance = agent.stopping_distance;
        if clamped(ui, "Stopping Distance:", &mut stopping_distance, 0.0..=50.0) {
            if let Some(mut a) = world.nav_agent_mut(id) {
                nav_ops::set_stopping_distance(&mut a, stopping_distance);
            }
            *is_dirty = true;
        }
        let mut radius = agent.radius;
        if clamped(ui, "Radius:", &mut radius, 0.01..=10.0) {
            if let Some(mut a) = world.nav_agent_mut(id) {
                nav_ops::set_radius(&mut a, radius);
            }
            *is_dirty = true;
        }
        draw_agent_avoidance(ui, world, id, &agent, is_dirty);
        draw_agent_target(ui, world, id, agent.target, is_dirty);
        ui.label(format!(
            "Velocity: [{:.2}, {:.2}, {:.2}]",
            agent.velocity.x, agent.velocity.y, agent.velocity.z
        ));
    });
    if remove {
        world.set_nav_agent(id, None);
        *is_dirty = true;
    }
}

/// The nav-agent local-avoidance toggle + priority (#463), through the shared ops.
fn draw_agent_avoidance(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    agent: &crate::components::NavMeshAgentComponent,
    is_dirty: &mut bool,
) {
    let mut enabled = agent.avoidance_enabled;
    if ui.checkbox(&mut enabled, "Avoid Other Agents").changed() {
        if let Some(mut a) = world.nav_agent_mut(id) {
            nav_ops::set_avoidance_enabled(&mut a, enabled);
        }
        *is_dirty = true;
    }
    let mut priority = agent.avoidance_priority;
    ui.horizontal(|ui| {
        ui.label("Avoidance Priority:");
        let max = crate::components::MAX_AVOIDANCE_PRIORITY;
        if ui
            .add(egui::DragValue::new(&mut priority).clamp_range(0..=max))
            .changed()
        {
            if let Some(mut a) = world.nav_agent_mut(id) {
                nav_ops::set_avoidance_priority(&mut a, f64::from(priority));
            }
            *is_dirty = true;
        }
    });
}

/// The nav-agent destination x/y/z row, routing a change through the shared op.
fn draw_agent_target(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    target: Vec3,
    is_dirty: &mut bool,
) {
    if let Some(t) = vec3_row(ui, "Target:", target) {
        if let Some(mut a) = world.nav_agent_mut(id) {
            nav_ops::set_target(&mut a, t);
        }
        *is_dirty = true;
    }
}

/// A labelled, clamped drag row (shared by the nav-agent scalar fields). Returns
/// whether the value changed.
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
