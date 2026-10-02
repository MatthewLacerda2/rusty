//! Fitting one hitbox per bone to the vertices that bone drives (#464).
//!
//! Each vertex belongs to its **dominant joint** (its heaviest skin weight). A
//! bone's hitbox is the bounding box of its vertices in the bone's own bind frame,
//! so it lines up with the bone however the skeleton is oriented. A bone too small
//! to be worth a hitbox (a finger, an eye, a twist bone) or one the caller did not
//! ask for hands its vertices to its parent: the hand's box then covers the
//! fingers, the head's covers the eyes and jaw. Pure data in, data out — the same
//! skin always yields the same boxes.

use glam::Vec3;

use crate::asset::SkinData;
use crate::components::mesh::Vertex;
use crate::components::{CapsuleAxis, ColliderShape};

/// Above this ratio between a bone's two cross-section extents the part is flat
/// (a chest, a pelvis, a hand) and gets a box; below it, a capsule.
const FLAT_RATIO: f32 = 1.5;

/// The smallest extent a generated box may have on any axis, in joint units, so
/// a single-plane cluster of vertices never yields a zero-thickness box.
const MIN_THICKNESS: f32 = 1.0e-3;

/// What `generate_hitboxes` builds and where.
#[derive(Clone, Debug, PartialEq)]
pub struct HitboxOptions {
    /// Only these bones get a hitbox (by joint name); `None` means every bone big
    /// enough. A bone left out hands its vertices to its parent.
    pub bones: Option<Vec<String>>,
    /// A bone whose vertices span less than this, in the model's own units
    /// (before the entity's scale), gets no hitbox of its own.
    pub min_size: f32,
    /// The layer the hitboxes go on, created (colliding with nothing) if absent.
    pub layer: String,
}

impl Default for HitboxOptions {
    fn default() -> Self {
        Self {
            bones: None,
            min_size: 0.08,
            layer: "Hitbox".to_string(),
        }
    }
}

/// One fitted hitbox: the joint slot it belongs to, its centre in the bone's
/// local frame, and its shape (sized in the bone's local units).
#[derive(Clone, Debug, PartialEq)]
pub struct HitboxFit {
    pub slot: usize,
    pub center: Vec3,
    pub shape: ColliderShape,
}

/// Fit the hitboxes of `skin` over `vertices`, ascending by joint slot.
pub fn fit_hitboxes(vertices: &[Vertex], skin: &SkinData, opts: &HitboxOptions) -> Vec<HitboxFit> {
    let joints = skin.local_bind.len();
    let mut points: Vec<Vec<Vec3>> = vec![Vec::new(); joints];
    for v in vertices {
        if let Some(slot) = dominant_joint(v).filter(|&s| s < joints) {
            points[slot].push(Vec3::from(v.position));
        }
    }
    let mut fits = Vec::new();
    // Parents come before children (the order `bind_bones` relies on), so a
    // reverse walk hands a child's vertices up before its parent is fitted.
    for slot in (0..joints).rev() {
        let pts = std::mem::take(&mut points[slot]);
        if pts.is_empty() {
            continue;
        }
        let wanted = opts
            .bones
            .as_ref()
            .is_none_or(|names| names.contains(&skin.name(slot)));
        match wanted.then(|| fit_slot(skin, slot, &pts, opts.min_size)) {
            Some(Some(fit)) => fits.push(fit),
            _ => {
                if let Some(parent) = skin.parents.get(slot).copied().flatten() {
                    if parent < slot {
                        points[parent].extend(pts);
                    }
                }
            }
        }
    }
    fits.reverse();
    fits
}

/// The joint a vertex follows most; ties go to the lower slot. `None` for a
/// vertex with no positive weight.
fn dominant_joint(v: &Vertex) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (&joint, &weight) in v.joint_indices.iter().zip(&v.joint_weights) {
        if weight > 0.0 && best.is_none_or(|(_, w)| weight > w) {
            best = Some((joint as usize, weight));
        }
    }
    best.map(|(joint, _)| joint)
}

/// Fit `slot`'s hitbox around `pts` (mesh-local), or `None` when it spans less
/// than `min_size` model units.
fn fit_slot(skin: &SkinData, slot: usize, pts: &[Vec3], min_size: f32) -> Option<HitboxFit> {
    let to_joint = *skin.inverse_bind.get(slot)?;
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in pts {
        let q = to_joint.transform_point3(*p);
        lo = lo.min(q);
        hi = hi.max(q);
    }
    let size = hi - lo;
    // Joint units → model units: the bone's bind scale (a skeleton exported at
    // 0.01 measures its bones in centimetres).
    let unit = skin
        .bind_global
        .get(slot)
        .map_or(1.0, |m| m.x_axis.truncate().length());
    if size.max_element() * unit < min_size {
        return None;
    }
    Some(HitboxFit {
        slot,
        center: (lo + hi) * 0.5,
        shape: shape_for(size),
    })
}

/// A capsule along the longest extent, or a box when the cross-section is flat.
fn shape_for(size: Vec3) -> ColliderShape {
    // Ties prefer Y: Blender bones point along their local +Y.
    let axis = [CapsuleAxis::Y, CapsuleAxis::X, CapsuleAxis::Z]
        .into_iter()
        .fold(CapsuleAxis::Y, |best, a| {
            if size.dot(a.unit()) > size.dot(best.unit()) {
                a
            } else {
                best
            }
        });
    let (along, a, b) = match axis {
        CapsuleAxis::X => (size.x, size.y, size.z),
        CapsuleAxis::Y => (size.y, size.x, size.z),
        CapsuleAxis::Z => (size.z, size.x, size.y),
    };
    let (wide, narrow) = (a.max(b), a.min(b));
    if narrow <= MIN_THICKNESS || wide / narrow > FLAT_RATIO {
        return ColliderShape::Box {
            size: size.max(Vec3::splat(MIN_THICKNESS)),
        };
    }
    let radius = (wide + narrow) * 0.25;
    ColliderShape::Capsule {
        radius,
        height: along.max(2.0 * radius),
        axis,
    }
}
