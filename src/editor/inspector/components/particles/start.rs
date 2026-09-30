//! src/editor/inspector/components/particles/start.rs — the shape, motion and
//! start-range sections of the Particle System card.

use glam::Vec3;

use super::widgets::{clamped, combo, range_row, vec3};
use super::{apply, Cx};
use crate::components::{EmitFrom, EmitShape, ParticleEmitterComponent};
use crate::core::curve::ColorRange;
use crate::scene::authoring::particles as particle_ops;

/// Emission shape: kind, volume/surface, and the kind's size knobs.
pub(super) fn draw_shape(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
    let (mut radius, mut angle, mut size) = shape_parts(p.shape);
    let mut name = p.shape.name();
    let options = EmitShape::NAMES.map(|n| (n, n));
    let mut changed = combo(ui, "Shape", &mut name, &options);
    match p.shape {
        EmitShape::Point => {}
        EmitShape::Box { .. } => changed |= vec3(ui, "Size:", &mut size, 0.05),
        EmitShape::Cone { .. } => {
            changed |= clamped(ui, "Angle (deg):", &mut angle, 0.0..=90.0);
            changed |= clamped(ui, "Radius:", &mut radius, 0.0..=100.0);
        }
        _ => changed |= clamped(ui, "Radius:", &mut radius, 0.0..=100.0),
    }
    if changed {
        if let Some(shape) = EmitShape::from_parts(name, radius, angle, size) {
            apply(cx, |c| particle_ops::set_shape(c, shape));
        }
    }
    let mut from = p.emit_from;
    let options = [(EmitFrom::Volume, "Volume"), (EmitFrom::Surface, "Surface")];
    if combo(ui, "Emit From", &mut from, &options) {
        apply(cx, |c| particle_ops::set_emit_from(c, from));
        changed = true;
    }
    changed
}

/// The size knobs of `shape` (radius, cone angle, box size), with defaults for
/// the ones it lacks so switching kinds starts from a visible size.
fn shape_parts(shape: EmitShape) -> (f32, f32, Vec3) {
    let (mut radius, mut angle, mut size) = (1.0, 25.0, Vec3::ONE);
    match shape {
        EmitShape::Point => {}
        EmitShape::Sphere { radius: r }
        | EmitShape::Hemisphere { radius: r }
        | EmitShape::Circle { radius: r } => radius = r,
        EmitShape::Box { size: s } => size = s,
        EmitShape::Cone {
            angle: a,
            radius: r,
        } => (angle, radius) = (a, r),
    }
    (radius, angle, size)
}

/// Per-particle motion: lifetime and speed ranges, direction, spread and gravity.
pub(super) fn draw_motion(
    ui: &mut egui::Ui,
    cx: &mut Cx<'_>,
    p: &ParticleEmitterComponent,
) -> bool {
    let mut changed = false;
    if let Some(r) = range_row(ui, "Lifetime (sec):", p.lifetime, 0.0..=60.0) {
        apply(cx, |c| particle_ops::set_lifetime(c, r));
        changed = true;
    }
    if let Some(r) = range_row(ui, "Speed:", p.speed, -100.0..=100.0) {
        apply(cx, |c| particle_ops::set_speed(c, r));
        changed = true;
    }
    let mut direction = p.direction;
    if vec3(ui, "Direction:", &mut direction, 0.05) {
        apply(cx, |c| particle_ops::set_direction(c, direction));
        changed = true;
    }
    let mut spread = p.spread;
    if clamped(ui, "Spread:", &mut spread, 0.0..=1.0) {
        apply(cx, |c| particle_ops::set_spread(c, spread));
        changed = true;
    }
    let mut gravity = p.gravity;
    if vec3(ui, "Gravity:", &mut gravity, 0.1) {
        apply(cx, |c| particle_ops::set_gravity(c, gravity));
        changed = true;
    }
    changed
}

/// Start size, rotation and the two-colour start tint.
pub(super) fn draw_start(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
    let mut changed = false;
    if let Some(r) = range_row(ui, "Start Size:", p.size, 0.0..=50.0) {
        apply(cx, |c| particle_ops::set_size(c, r));
        changed = true;
    }
    if let Some(r) = range_row(ui, "Start Rotation (deg):", p.rotation, -360.0..=360.0) {
        apply(cx, |c| particle_ops::set_rotation(c, r));
        changed = true;
    }
    let ColorRange { mut min, mut max } = p.color;
    ui.horizontal(|ui| {
        ui.label("Start Color (min / max):");
        let a = ui.color_edit_button_rgba_unmultiplied(&mut min).changed();
        let b = ui.color_edit_button_rgba_unmultiplied(&mut max).changed();
        if a | b {
            apply(cx, |c| particle_ops::set_color(c, ColorRange { min, max }));
            changed = true;
        }
    });
    changed
}
