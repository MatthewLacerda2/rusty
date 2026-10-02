//! src/editor/viewport/rect_tool/overlay.rs — painting the rect tool (#423).
//!
//! Over the Scene image, with the UI overlay on: each screen canvas's outline,
//! and for a selected UI element its quad, the eight resize handles, the pivot
//! disc and the four anchor triangles. A read-only element (see the module docs
//! of `rect_tool`) is drawn in the muted colour with no handles.

use egui::{Color32, Painter, Pos2, Shape, Stroke};
use glam::Vec2;

use super::geometry::{self, Handle};
use super::{target, OverlayFrame, Target};
use crate::ecs::World;
use crate::editor::theme::Theme;
use crate::ui::CanvasSpace;

/// Paint the overlay for `world` into `painter`, the viewport image's top-left at
/// `origin`.
pub fn paint(
    painter: &Painter,
    origin: Pos2,
    frame: &OverlayFrame,
    world: &World,
    selected: Option<u32>,
    t: &Theme,
) {
    let layout = frame.layout(world);
    let at = |c: Vec2, scale: f32| {
        let p = frame.to_local(c, scale);
        origin + egui::vec2(p.x, p.y)
    };
    for (canvas, space) in layout.canvases() {
        let Some(r) = layout.get(canvas).filter(|_| space == CanvasSpace::Screen) else {
            continue;
        };
        let quad = r.corners.map(|c| at(c, r.scale_factor)).to_vec();
        painter.add(Shape::closed_line(quad, Stroke::new(1.0, t.border)));
    }
    let Some(target) = selected.and_then(|id| target(world, &layout, id)) else {
        return;
    };
    let colour = if target.editable {
        t.accent
    } else {
        t.text_secondary
    };
    let scale = target.rect.scale_factor;
    let quad = target.rect.corners.map(|c| at(c, scale)).to_vec();
    painter.add(Shape::closed_line(quad, Stroke::new(1.5, colour)));
    paint_anchors(painter, &target, |c| at(c, scale), colour);
    if target.editable {
        paint_handles(painter, &target, |c| at(c, scale), colour, t.bg_tier0);
    }
}

/// The resize squares and the pivot disc.
fn paint_handles(
    painter: &Painter,
    target: &Target,
    at: impl Fn(Vec2) -> Pos2,
    colour: Color32,
    fill: Color32,
) {
    for (handle, c) in geometry::handles(&target.rect, target.rt.pivot) {
        let p = at(c);
        match handle {
            Handle::Pivot => {
                painter.circle_stroke(p, 5.0, Stroke::new(1.5, colour));
            }
            _ => {
                let r = egui::Rect::from_center_size(p, egui::vec2(7.0, 7.0));
                painter.rect_filled(r, 0.0, fill);
                painter.rect_stroke(r, 0.0, Stroke::new(1.0, colour));
            }
        }
    }
}

/// Unity's anchor triangles: one per anchor corner, tip on the anchor, pointing in
/// from outside the anchor box.
fn paint_anchors(painter: &Painter, target: &Target, at: impl Fn(Vec2) -> Pos2, colour: Color32) {
    if target.parent == target.rect {
        return; // a root canvas has no parent to anchor in
    }
    let Some(points) = geometry::anchor_points(&target.rt, &target.parent) else {
        return;
    };
    // Outward, in viewport points (y down): bottom-left, top-left, top-right,
    // bottom-right.
    let out = [(-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (1.0, 1.0)];
    for (c, (dx, dy)) in points.into_iter().zip(out) {
        let tip = at(c);
        let d = egui::vec2(dx, dy).normalized();
        let side = egui::vec2(-d.y, d.x) * 4.0;
        let base = tip + d * 10.0;
        let tri = vec![tip, base + side, base - side];
        painter.add(Shape::convex_polygon(
            tri,
            Color32::WHITE,
            Stroke::new(1.0, colour),
        ));
    }
}
