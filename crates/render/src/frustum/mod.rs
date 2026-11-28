use glam::{Mat4, Vec3, Vec4};

pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    pub fn from_view_proj(view_proj: Mat4) -> Self {
        let r0 = view_proj.row(0);
        let r1 = view_proj.row(1);
        let r2 = view_proj.row(2);
        let r3 = view_proj.row(3);
        Self {
            planes: [
                r3 + r0, // left
                r3 - r0, // right
                r3 + r1, // bottom
                r3 - r1, // top
                // Depth's 0..w: a camera's far and near plane (reversed, `camera::depth`), a
                // light's near and far one (ordinary), so one frustum serves both.
                r2,
                r3 - r2,
            ],
        }
    }

    /// Monotone in the box (a box holding a passing box passes): the caster BVH
    /// (`scene/casters`) relies on it.
    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            let n = plane.truncate();
            let p_vertex = Vec3::new(
                if n.x >= 0.0 { max.x } else { min.x },
                if n.y >= 0.0 { max.y } else { min.y },
                if n.z >= 0.0 { max.z } else { min.z },
            );
            if n.dot(p_vertex) + plane.w < 0.0 {
                return false;
            }
        }
        true
    }
}

pub fn transform_aabb(local_min: Vec3, local_max: Vec3, world: Mat4) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for i in 0..8 {
        let corner = Vec3::new(
            if i & 1 == 0 { local_min.x } else { local_max.x },
            if i & 2 == 0 { local_min.y } else { local_max.y },
            if i & 4 == 0 { local_min.z } else { local_max.z },
        );
        let w = world.transform_point3(corner);
        min = min.min(w);
        max = max.max(w);
    }
    (min, max)
}

mod skinned;
mod small;

pub use skinned::SkinBounds;
pub(crate) use small::{SmallLimits, SmallObjectCull};

#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod tests;
