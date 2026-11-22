//! crates/editor/src/viewport/rect_tool/geometry.rs — the rect tool's pure maths.

use glam::{Affine2, Mat2, Vec2};

use rusty_components::RectTransformComponent;
use rusty_ui::UiRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// Inside the rect: move it (`anchored_position`).
    Body,
    /// The pivot.
    Pivot,
    /// An edge or corner: -1 = min, +1 = max.
    Resize { x: i8, y: i8 },
}

const RESIZE: [(i8, i8); 8] = [
    (-1, -1),
    (-1, 1),
    (1, 1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (0, -1),
    (0, 1),
];

pub fn frame_of(r: &UiRect) -> Option<Affine2> {
    let (min, size) = r.rect;
    if size.x.abs() < 1e-6 || size.y.abs() < 1e-6 {
        return None;
    }
    let c = r.corners;
    let m = Mat2::from_cols((c[3] - c[0]) / size.x, (c[1] - c[0]) / size.y);
    (m.determinant().abs() > 1e-9).then(|| Affine2::from_mat2_translation(m, c[0] - m * min))
}

/// Where each handle of `r` sits, in canvas units: the pivot, then the resize
/// handles in hit priority order.
pub fn handles(r: &UiRect, pivot: Vec2) -> Vec<(Handle, Vec2)> {
    let Some(f) = frame_of(r) else {
        return Vec::new();
    };
    let (min, size) = r.rect;
    let at = |t: Vec2| f.transform_point2(min + size * t);
    let mut out = vec![(Handle::Pivot, at(pivot))];
    for (x, y) in RESIZE {
        let t = Vec2::new(f32::from(x) + 1.0, f32::from(y) + 1.0) * 0.5;
        out.push((Handle::Resize { x, y }, at(t)));
    }
    out
}

pub fn hit(
    r: &UiRect,
    pivot: Vec2,
    to: impl Fn(Vec2) -> Vec2,
    p: Vec2,
    radius: f32,
) -> Option<Handle> {
    let near = handles(r, pivot)
        .into_iter()
        .find(|&(_, at)| to(at).distance(p) <= radius);
    if let Some((h, _)) = near {
        return Some(h);
    }
    inside(&r.corners.map(to), p).then_some(Handle::Body)
}

fn inside(c: &[Vec2; 4], p: Vec2) -> bool {
    let sides = (0..4).map(|i| (c[(i + 1) % 4] - c[i]).perp_dot(p - c[i]));
    let (mut pos, mut neg) = (false, false);
    for s in sides {
        pos |= s > 0.0;
        neg |= s < 0.0;
    }
    !(pos && neg)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectEdit {
    pub anchored_position: Vec2,
    pub size_delta: Vec2,
    pub pivot: Vec2,
}

/// The edit a drag of `delta` (canvas units) on `handle` makes to `rt`, laid out
/// as `own` inside `parent`. `None` when either frame is degenerate.
pub fn drag(
    rt: &RectTransformComponent,
    handle: Handle,
    delta: Vec2,
    own: &UiRect,
    parent: &UiRect,
) -> Option<RectEdit> {
    let own_frame = frame_of(own)?.matrix2;
    let to_parent = frame_of(parent)?.matrix2.inverse();
    let local = own_frame.inverse() * delta;
    let mut edit = RectEdit {
        anchored_position: rt.anchored_position,
        size_delta: rt.size_delta,
        pivot: rt.pivot,
    };
    match handle {
        Handle::Body => edit.anchored_position += to_parent * delta,
        Handle::Pivot => {
            edit.anchored_position += to_parent * delta;
            edit.pivot += local / own.rect.1;
        }
        Handle::Resize { x, y } => {
            let mut shift = Vec2::ZERO;
            for (axis, side) in [x, y].into_iter().enumerate() {
                let d = local[axis];
                let (grow, share) = match side {
                    1 => (d, rt.pivot[axis]),
                    -1 => (-d, 1.0 - rt.pivot[axis]),
                    _ => continue,
                };
                edit.size_delta[axis] += grow;
                shift[axis] = d * share;
            }
            edit.anchored_position += to_parent * (own_frame * shift);
        }
    }
    Some(edit)
}

/// The four anchor points of `rt` inside `parent`, in canvas units: bottom-left,
/// top-left, top-right, bottom-right (one point four times for point anchors).
pub fn anchor_points(rt: &RectTransformComponent, parent: &UiRect) -> Option<[Vec2; 4]> {
    let f = frame_of(parent)?;
    let (min, size) = parent.rect;
    let (lo, hi) = (rt.anchor_min, rt.anchor_max);
    let at = |x: f32, y: f32| f.transform_point2(min + size * Vec2::new(x, y));
    Some([
        at(lo.x, lo.y),
        at(lo.x, hi.y),
        at(hi.x, hi.y),
        at(hi.x, lo.y),
    ])
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod geometry_tests;
