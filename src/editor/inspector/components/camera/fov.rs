//! The Camera card's field of view (#2038): the *FOV Axis* dropdown and the
//! slider, labelled with its axis and editing in it, with the other axis's angle
//! at the view's aspect beneath, as Unity's `CameraEditor` does. The stored `fov`
//! stays vertical; every write routes through the shared ops.

use crate::components::{CameraComponent, FovAxis};
use crate::scene::authoring::rendering::camera as camera_ops;

/// The aspect (width / height) the card converts at: the camera's render
/// texture's when it has one, else `view` (the editor viewport's, which the Scene
/// and Game tabs share), else 16:9 (no viewport drawn yet).
pub(super) fn aspect(cam: &CameraComponent, view: f32) -> f32 {
    let target = cam.target_texture.as_ref();
    let aspect = target.map_or(view, |t| t.width as f32 / t.height.max(1) as f32);
    if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        16.0 / 9.0
    }
}

/// The *FOV Axis* dropdown and the field of view along it.
pub(super) fn draw_fov(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    cam: &CameraComponent,
    aspect: f32,
    is_dirty: &mut bool,
) {
    let mut axis = cam.fov_axis;
    ui.horizontal(|ui| {
        ui.label("FOV Axis:");
        egui::ComboBox::from_id_salt("fov_axis")
            .selected_text(axis.name())
            .show_ui(ui, |ui| {
                for value in FovAxis::ALL {
                    if ui
                        .selectable_value(&mut axis, value, value.name())
                        .changed()
                    {
                        if let Some(mut c) = world.camera_mut(id) {
                            camera_ops::set_fov_axis(&mut c, axis);
                        }
                        *is_dirty = true;
                    }
                }
            });
    });
    let (label, other, other_axis) = match axis {
        FovAxis::Vertical => ("vertical", FovAxis::Horizontal, "horizontal"),
        FovAxis::Horizontal => ("horizontal", FovAxis::Vertical, "vertical"),
    };
    let range = axis.show(camera_ops::MIN_FOV, aspect)..=axis.show(camera_ops::MAX_FOV, aspect);
    // The held angle, as the lens draws it: a hand-edited scene's out-of-range
    // `fov` would convert to nonsense (tan past 90° is negative).
    let held = cam.fov.clamp(camera_ops::MIN_FOV, camera_ops::MAX_FOV);
    let mut fov = axis.show(held, aspect);
    ui.horizontal(|ui| {
        ui.label(format!("Field of View ({label}):"));
        if ui
            .add(crate::editor::widgets::slider(&mut fov, range))
            .changed()
        {
            if let Some(mut c) = world.camera_mut(id) {
                // Converted to the stored vertical angle, as `Camera.SetFieldOfView`
                // takes it (`HorizontalToVerticalFov` first, for a script).
                camera_ops::set_fov(&mut c, axis.store(fov, aspect));
            }
            *is_dirty = true;
        }
    });
    let equivalent = other.show(axis.store(fov, aspect), aspect);
    ui.weak(format!("= {equivalent:.1}° {other_axis} at {aspect:.2}:1"));
}
