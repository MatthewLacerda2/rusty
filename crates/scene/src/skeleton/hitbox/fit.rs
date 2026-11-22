//! A bone too small or not asked for hands its vertices to its parent.
use glam::Vec3;

use rusty_asset::SkinData;
use rusty_components::mesh::Vertex;
use rusty_components::{CapsuleAxis, ColliderShape};

/// Above this ratio of the two cross-section extents a part is flat (a chest) and gets a box.
const FLAT_RATIO: f32 = 1.5;

/// Joint units, so a flat cluster never yields a zero-thickness box.
const MIN_THICKNESS: f32 = 1.0e-3;

#[derive(Clone, Debug, PartialEq)]
pub struct HitboxOptions {
    pub bones: Option<Vec<String>>,
    /// In the model's own units, before the entity's scale.
    pub min_size: f32,
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

#[derive(Clone, Debug, PartialEq)]
pub struct HitboxFit {
    pub slot: usize,
    pub center: Vec3,
    pub shape: ColliderShape,
}

pub fn fit_hitboxes(vertices: &[Vertex], skin: &SkinData, opts: &HitboxOptions) -> Vec<HitboxFit> {
    let joints = skin.local_bind.len();
    let mut points: Vec<Vec<Vec3>> = vec![Vec::new(); joints];
    for v in vertices {
        if let Some(slot) = dominant_joint(v).filter(|&s| s < joints) {
            points[slot].push(Vec3::from(v.position));
        }
    }
    let mut fits = Vec::new();
    // Parents come first, so walking back hands a child's vertices up before its parent fits.
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

fn dominant_joint(v: &Vertex) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (&joint, &weight) in v.joint_indices.iter().zip(&v.joint_weights) {
        if weight > 0.0 && best.is_none_or(|(_, w)| weight > w) {
            best = Some((joint as usize, weight));
        }
    }
    best.map(|(joint, _)| joint)
}

fn fit_slot(skin: &SkinData, slot: usize, pts: &[Vec3], min_size: f32) -> Option<HitboxFit> {
    let to_joint = *skin.inverse_bind.get(slot)?;
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in pts {
        let q = to_joint.transform_point3(*p);
        lo = lo.min(q);
        hi = hi.max(q);
    }
    let size = hi - lo;
    // Joint units to model units: the bone's bind scale (a 0.01 skeleton measures in cm).
    let unit = skin.bind_global.get(slot).map_or(1.0, |m| {
        (skin.mesh_to_model() * *m).x_axis.truncate().length()
    });
    if size.max_element() * unit < min_size {
        return None;
    }
    Some(HitboxFit {
        slot,
        center: (lo + hi) * 0.5,
        shape: shape_for(size),
    })
}

fn shape_for(size: Vec3) -> ColliderShape {
    let axis = [CapsuleAxis::Y, CapsuleAxis::X, CapsuleAxis::Z]
        .into_iter()
        // Ties prefer Y: Blender bones point along their local +Y.
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
    // The mean of the two cross-section half-extents.
    let radius = (wide + narrow) * 0.25;
    ColliderShape::Capsule {
        radius,
        height: along.max(2.0 * radius),
        axis,
    }
}
