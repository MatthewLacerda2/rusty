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

/// A shared `authoring::nav_agent` scalar setter, one per clamped drag row.
type NavOp = fn(&mut crate::components::NavMeshAgentComponent, f32);

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
        let rows: [(&str, f32, _, NavOp); 5] = [
            ("Speed:", agent.speed, 0.0..=100.0, nav_ops::set_speed),
            (
                "Acceleration:",
                agent.acceleration,
                0.0..=100.0,
                nav_ops::set_acceleration,
            ),
            (
                "Stopping Distance:",
                agent.stopping_distance,
                0.0..=50.0,
                nav_ops::set_stopping_distance,
            ),
            ("Radius:", agent.radius, 0.01..=10.0, nav_ops::set_radius),
            (
                "Base Offset:",
                agent.base_offset,
                -10.0..=10.0,
                nav_ops::set_base_offset,
            ),
        ];
        for (label, mut value, range, op) in rows {
            if clamped(ui, label, &mut value, range) {
                if let Some(mut a) = world.nav_agent_mut(id) {
                    op(&mut a, value);
                }
                *is_dirty = true;
            }
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

/// The nav-agent local-avoidance toggle + priority (#463) and the off-mesh link
/// auto-traverse toggle (#462), through the shared ops.
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
    let mut auto = agent.auto_traverse_off_mesh_link;
    let label = "Auto Traverse Off-Mesh Link";
    if ui.checkbox(&mut auto, label).changed() {
        if let Some(mut a) = world.nav_agent_mut(id) {
            nav_ops::set_auto_traverse_off_mesh_link(&mut a, auto);
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
