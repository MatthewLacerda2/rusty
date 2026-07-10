//! src/editor/inspector/components/gameplay/collider.rs — the Collider inspector
//! card. Split out of the physics-card module (`physics.rs`) to keep both files
//! under the size cap; the RigidBody and NavMesh Agent cards stay in `physics`.
//!
//! A THIN client (#287): widgets read fields from a snapshot and route every write
//! through a shared `authoring::collider` op, never a mutable component borrow — the
//! same ops the `Physics.*` Lua setters call, so the panel and the bindings share
//! one write.

use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::collider as collider_ops;
use crate::scene::ColliderShape;

pub fn draw_collider(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
    pending_nav_bake: &mut bool,
) {
    let Some(collider) = world.collider(id).map(|c| c.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(
        ui,
        icon::BOUNDING_BOX,
        "Collider",
        Some(&mut remove),
        |ui| {
            let mut active = collider.active;
            if ui.checkbox(&mut active, "Active").changed() {
                if let Some(mut c) = world.collider_mut(id) {
                    collider_ops::set_active(&mut c, active);
                }
                *is_dirty = true;
            }
            let mut is_trigger = collider.is_trigger;
            if ui.checkbox(&mut is_trigger, "Is Trigger").changed() {
                if let Some(mut c) = world.collider_mut(id) {
                    collider_ops::set_trigger(&mut c, is_trigger);
                }
                *is_dirty = true;
            }
            // The shape (variant + extents) is edited in a local clone, then routed
            // through the shared op as a whole when any widget changes it.
            let mut shape = collider.shape.clone();
            let mut shape_changed = draw_shape_selector(ui, &mut shape, pending_nav_bake);
            shape_changed |= draw_shape_fields(ui, &mut shape, pending_nav_bake);
            if shape_changed {
                if let Some(mut c) = world.collider_mut(id) {
                    collider_ops::set_shape(&mut c, shape);
                }
            }
        },
    );
    if remove {
        world.set_collider(id, None);
        *is_dirty = true;
    }
}

/// The Shape combo box; switching to a new shape resets it to sensible defaults
/// and requests a nav rebake (the static geometry changed). Mutates the local
/// `shape` clone; returns whether the shape changed.
fn draw_shape_selector(
    ui: &mut egui::Ui,
    shape: &mut ColliderShape,
    pending_nav_bake: &mut bool,
) -> bool {
    let mut shape_type = match shape {
        ColliderShape::Box { .. } => "Box",
        ColliderShape::Sphere { .. } => "Sphere",
        ColliderShape::Cylinder { .. } => "Cylinder",
        ColliderShape::Mesh { .. } => "Mesh",
    };
    let old_shape_type = shape_type;
    egui::ComboBox::from_label("Shape")
        .selected_text(shape_type)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut shape_type, "Box", "Box");
            ui.selectable_value(&mut shape_type, "Sphere", "Sphere");
            ui.selectable_value(&mut shape_type, "Cylinder", "Cylinder");
        });
    if shape_type != old_shape_type {
        *shape = match shape_type {
            "Sphere" => ColliderShape::Sphere { radius: 0.5 },
            "Cylinder" => ColliderShape::Cylinder {
                radius: 0.5,
                height: 1.0,
            },
            _ => ColliderShape::Box { size: Vec3::ONE },
        };
        *pending_nav_bake = true;
        return true;
    }
    false
}

/// The shape-specific extent fields (or the baked-mesh hull toggle). Mutates the
/// local `shape` clone; returns whether any extent changed.
fn draw_shape_fields(
    ui: &mut egui::Ui,
    shape: &mut ColliderShape,
    pending_nav_bake: &mut bool,
) -> bool {
    match shape {
        ColliderShape::Box { size } => {
            ui.horizontal(|ui| {
                ui.label("Size:");
                let mut c = drag(ui, &mut size.x);
                c |= drag(ui, &mut size.y);
                c |= drag(ui, &mut size.z);
                c
            })
            .inner
        }
        ColliderShape::Sphere { radius } => {
            ui.horizontal(|ui| {
                ui.label("Radius:");
                drag(ui, radius)
            })
            .inner
        }
        ColliderShape::Cylinder { radius, height } => {
            ui.horizontal(|ui| {
                ui.label("Radius:");
                let mut c = drag(ui, radius);
                ui.label("Height:");
                c |= drag(ui, height);
                c
            })
            .inner
        }
        ColliderShape::Mesh { convex, .. } => {
            // Baked from the imported mesh (#77): no editable extents, but
            // the hull/trimesh choice can still be flipped here.
            ui.label("Baked from imported mesh");
            if ui.checkbox(convex, "Convex hull").changed() {
                *pending_nav_bake = true;
                return true;
            }
            false
        }
    }
}

/// A clamped collider-dimension drag field (shared between the shape variants).
/// Returns whether the value changed.
fn drag(ui: &mut egui::Ui, value: &mut f32) -> bool {
    ui.add(
        egui::DragValue::new(value)
            .speed(0.05)
            .clamp_range(0.01..=100.0),
    )
    .changed()
}
