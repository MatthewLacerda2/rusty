use crate::inspector::components::card::remove_component;
use crate::widgets::DragRange;
use egui_phosphor::regular as icon;
use glam::Vec3;
use rusty_edit::authoring::ComponentKind;

use crate::inspector::components::card::component_card;
use rusty_edit::authoring::physics::rigidbody as rigidbody_ops;
use rusty_scene::CollisionDetection;

fn rb_checkbox(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    label: &str,
    mut value: bool,
    op: fn(&mut rusty_scene::RigidBodyComponent, bool),
) {
    if ui.checkbox(&mut value, label).changed() {
        if let Some(mut r) = world.rigidbody_mut(id) {
            op(&mut r, value);
        }
    }
}

/// The Mass drag row (clamped ≥ 0.01), routed through the shared op.
fn draw_rb_mass(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, mut mass: f32) {
    ui.horizontal(|ui| {
        ui.label("Mass:");
        let drag = egui::DragValue::new(&mut mass)
            .speed(0.05)
            .ranged(0.01..=1000.0);
        if ui.add(drag).changed() {
            if let Some(mut r) = world.rigidbody_mut(id) {
                rigidbody_ops::set_mass(&mut r, mass);
            }
        }
    });
}

/// A Drag / Angular Drag row (≥ 0, Unity's `drag` / `angularDrag`), routed
/// through the shared op.
fn draw_rb_drag(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    (label, mut value): (&str, f32),
    op: fn(&mut rusty_scene::RigidBodyComponent, f32),
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let drag = egui::DragValue::new(&mut value)
            .speed(0.01)
            .ranged(0.0..=f32::MAX);
        if ui.add(drag).changed() {
            if let Some(mut r) = world.rigidbody_mut(id) {
                op(&mut r, value);
            }
        }
    });
}

/// The Drag and Angular Drag rows.
fn draw_rb_drags(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    (drag, angular): (f32, f32),
) {
    let set = rigidbody_ops::set_drag;
    draw_rb_drag(ui, world, id, ("Drag:", drag), set);
    let set = rigidbody_ops::set_angular_drag;
    draw_rb_drag(ui, world, id, ("Angular Drag:", angular), set);
}

pub fn draw_rigidbody(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32) {
    let Some(rb) = world.rigidbody(id).map(|rb| rb.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::CUBE, "RigidBody", Some(&mut remove), |ui| {
        rb_checkbox(
            ui,
            world,
            id,
            "Active",
            rb.active,
            rigidbody_ops::set_active,
        );
        rb_checkbox(
            ui,
            world,
            id,
            "Is Kinematic",
            rb.is_kinematic,
            rigidbody_ops::set_kinematic,
        );
        rb_checkbox(
            ui,
            world,
            id,
            "Use Gravity",
            rb.use_gravity,
            rigidbody_ops::set_use_gravity,
        );
        draw_rb_mass(ui, world, id, rb.mass);
        draw_rb_drags(ui, world, id, (rb.drag, rb.angular_drag));
        draw_rb_velocity(ui, world, id, rb.velocity);
        draw_rb_angular_velocity(ui, world, id, rb.angular_velocity);
        draw_rb_collision_detection(ui, world, id, rb.collision_detection);
    });
    if remove {
        remove_component(world, id, ComponentKind::RigidBody);
    }
}

/// The rigidbody velocity x/y/z row, routing a change through the shared op.
fn draw_rb_velocity(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, velocity: Vec3) {
    if let Some(v) = vec3_row(ui, "Velocity:", velocity) {
        if let Some(mut r) = world.rigidbody_mut(id) {
            rigidbody_ops::set_velocity(&mut r, v);
        }
    }
}

/// The rigidbody angular-velocity x/y/z row (radians/sec per axis), routing a
/// change through the shared op.
fn draw_rb_angular_velocity(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    angular_velocity: Vec3,
) {
    if let Some(w) = vec3_row(ui, "Angular Velocity:", angular_velocity) {
        if let Some(mut r) = world.rigidbody_mut(id) {
            rigidbody_ops::set_angular_velocity(&mut r, w);
        }
    }
}

fn draw_rb_collision_detection(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    current: CollisionDetection,
) {
    let mut mode = current;
    egui::ComboBox::from_label("Collision Detection")
        .selected_text(mode.name())
        .show_ui(ui, |ui| {
            for m in CollisionDetection::ALL {
                ui.selectable_value(&mut mode, m, m.name());
            }
        });
    if mode != current {
        if let Some(mut r) = world.rigidbody_mut(id) {
            rigidbody_ops::set_collision_detection(&mut r, mode);
        }
    }
}

pub(super) fn vec3_row(ui: &mut egui::Ui, label: &str, value: Vec3) -> Option<Vec3> {
    let mut value = value;
    let changed = ui
        .horizontal(|ui| {
            ui.label(label);
            let mut c = ui
                .add(egui::DragValue::new(&mut value.x).speed(0.1))
                .changed();
            c |= ui
                .add(egui::DragValue::new(&mut value.y).speed(0.1))
                .changed();
            c |= ui
                .add(egui::DragValue::new(&mut value.z).speed(0.1))
                .changed();
            c
        })
        .inner;
    changed.then_some(value)
}

impl crate::inspector::components::add::AddEntry for rusty_components::RigidBodyComponent {
    const CATEGORY: crate::inspector::components::add::Category =
        crate::inspector::components::add::Category::Physics;
    const LABEL: &'static str = "RigidBody Component";
}
