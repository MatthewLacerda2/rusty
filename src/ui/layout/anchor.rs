//! src/ui/layout/anchor.rs — world-anchored markers (#429).
//!
//! A RectTransform with a `WorldAnchor` is placed by the camera, not its anchors:
//! each layout pass projects the target's world position (+ offset) to the screen
//! and pins the element's pivot there — Unity's `WorldToScreenPoint` + a
//! `RectTransform` write, done as a layout rule instead of per-frame script maths.
//!
//! Off-screen behaviour, in order:
//! 1. **Behind the camera** with `hide_when_behind` → hidden (with its children).
//! 2. **On screen** (in front, inside the edge padding) → at the projected point,
//!    upright.
//! 3. **Off screen or behind** with `clamp_to_screen_edge` → on the padded screen
//!    edge, along the ray from the screen centre toward the target (a target behind
//!    the camera projects mirrored, so its direction is flipped back); with
//!    `rotate_toward_target` the element turns so its up points that way.
//! 4. Otherwise → at the raw projected point (off the canvas, or mirrored).
//!
//! Needs a camera: a view without one leaves markers where their anchors put them.
//! A target that no longer exists hides the marker.

use glam::{Vec2, Vec3};

use crate::components::WorldAnchor;
use crate::ecs::World;
use crate::ui::space::world_matrix;
use crate::ui::UiView;

/// Where a marker goes this pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Placement {
    /// Not laid out (nor its children): not drawn, not hit.
    Hidden,
    /// Its pivot at `pivot` (canvas reference units), turned by `angle` radians
    /// (counter-clockwise).
    At { pivot: Vec2, angle: f32 },
}

/// Where `anchor` puts its marker on a screen canvas `canvas_size` reference units
/// big at `scale` pixels per unit, seen through `view`; `None` without a camera.
pub(super) fn place(
    world: &World,
    anchor: &WorldAnchor,
    view: &UiView,
    canvas_size: Vec2,
    scale: f32,
) -> Option<Placement> {
    let cam = view.camera.as_ref()?;
    let point = match anchor.target {
        Some(t) if world.transform(t).is_none() => return Some(Placement::Hidden),
        Some(t) => world_matrix(world, t).transform_point3(Vec3::ZERO) + anchor.offset,
        None => anchor.offset,
    };
    let sp = cam.world_to_screen(point, view.screen);
    Some(clamp(
        anchor,
        sp.position / scale,
        sp.is_behind(),
        canvas_size,
    ))
}

/// The placement of a marker projected to `p` (canvas units) on a `size` canvas.
pub(super) fn clamp(anchor: &WorldAnchor, p: Vec2, behind: bool, size: Vec2) -> Placement {
    if behind && anchor.hide_when_behind {
        return Placement::Hidden;
    }
    let pad = Vec2::splat(anchor.edge_padding.max(0.0)).min(size * 0.5);
    let inside = p.cmpge(pad).all() && p.cmple(size - pad).all();
    if (inside && !behind) || !anchor.clamp_to_screen_edge {
        return Placement::At {
            pivot: p,
            angle: 0.0,
        };
    }
    let centre = size * 0.5;
    let mut dir = if behind { centre - p } else { p - centre };
    if dir.length_squared() < 1e-12 {
        dir = Vec2::NEG_Y; // dead behind: straight down, where "behind you" reads
    }
    let half = centre - pad;
    let t = [0, 1]
        .map(|a| match dir[a].abs() {
            d if d > 1e-12 => half[a] / d,
            _ => f32::INFINITY,
        })
        .into_iter()
        .fold(f32::INFINITY, f32::min);
    let angle = match anchor.rotate_toward_target {
        true => dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2,
        false => 0.0,
    };
    Placement::At {
        pivot: centre + dir * t,
        angle,
    }
}

#[cfg(test)]
#[path = "anchor_tests.rs"]
mod anchor_tests;
