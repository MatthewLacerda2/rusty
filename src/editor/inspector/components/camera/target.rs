//! The Camera card's projection mode and render-texture target (#430). Every write
//! routes through `scene::authoring::camera`, the ops the `Camera.*` per-entity
//! functions use.

use crate::components::{CameraComponent, Projection};
use crate::scene::authoring::camera as camera_ops;

/// Perspective / orthographic, and the orthographic half-height.
pub(super) fn draw_projection_mode(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    cam: &CameraComponent,
    is_dirty: &mut bool,
) {
    let mut next = None;
    ui.horizontal(|ui| {
        ui.label("Projection:");
        egui::ComboBox::from_id_source("camera_projection")
            .selected_text(camera_ops::projection_name(cam.projection))
            .show_ui(ui, |ui| {
                let size = match cam.projection {
                    Projection::Orthographic { size } => size,
                    Projection::Perspective => 5.0,
                };
                for p in [Projection::Perspective, Projection::Orthographic { size }] {
                    let label = camera_ops::projection_name(p);
                    if ui.selectable_label(cam.projection == p, label).clicked() {
                        next = Some(p);
                    }
                }
            });
    });
    if let Projection::Orthographic { size } = cam.projection {
        let mut size = size;
        ui.horizontal(|ui| {
            ui.label("Ortho Size:");
            let drag = egui::DragValue::new(&mut size)
                .speed(0.1)
                .clamp_range(0.01..=1000.0);
            if ui.add(drag).changed() {
                next = Some(Projection::Orthographic { size });
            }
        });
    }
    if let Some(p) = next {
        if let Some(mut c) = world.camera_mut(id) {
            camera_ops::set_projection(&mut c, p);
        }
        *is_dirty = true;
    }
}

/// The render-texture target: a name (empty = the screen), its size, post-FX and
/// update rate. Shown to consumers as `rt:<name>`.
pub(super) fn draw_target(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    cam: &CameraComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    let target = cam.target_texture.clone();
    let (mut name, mut w, mut h) = target.as_ref().map_or((String::new(), 256, 256), |t| {
        (t.name.clone(), t.width, t.height)
    });
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Target Texture:");
        changed |= ui.text_edit_singleline(&mut name).changed();
    });
    let Some(t) = target else {
        ui.weak("Empty: renders to the screen. Named: shows as rt:<name>.");
        if changed {
            write(world, id, is_dirty, |c| {
                camera_ops::set_target_texture(c, &name, w, h)
            });
        }
        return;
    };
    ui.horizontal(|ui| {
        ui.label("Size:");
        let max = camera_ops::MAX_TARGET_SIZE;
        changed |= ui
            .add(egui::DragValue::new(&mut w).clamp_range(1..=max))
            .changed();
        changed |= ui
            .add(egui::DragValue::new(&mut h).clamp_range(1..=max))
            .changed();
    });
    if changed {
        write(world, id, is_dirty, |c| {
            camera_ops::set_target_texture(c, &name, w, h)
        });
    }
    let mut post_fx = t.post_fx;
    if ui.checkbox(&mut post_fx, "Target Post-FX").changed() {
        write(world, id, is_dirty, |c| {
            camera_ops::set_target_post_fx(c, post_fx)
        });
    }
    let mut every = t.update_every;
    ui.horizontal(|ui| {
        ui.label("Update Every N Frames:");
        let drag = egui::DragValue::new(&mut every).clamp_range(1..=120);
        if ui.add(drag).changed() {
            write(world, id, is_dirty, |c| {
                camera_ops::set_target_update_every(c, every)
            });
        }
    });
}

/// Run `f` on `id`'s camera and mark the scene dirty.
fn write(
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
    f: impl FnOnce(&mut CameraComponent),
) {
    if let Some(mut c) = world.camera_mut(id) {
        f(&mut c);
    }
    *is_dirty = true;
}
