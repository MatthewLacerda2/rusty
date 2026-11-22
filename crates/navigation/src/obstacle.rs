use glam::{Mat4, Vec2, Vec3};

use super::avoidance::AvoidanceAgent;
use rusty_components::{NavMeshObstacleComponent as Obstacle, ObstacleShape};
use rusty_scene::Scene;

const ROUND_SEGMENTS: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct ObstacleVolume {
    pub shape: ObstacleShape,
    pub center: Vec3,
    pub size: Vec3,
    pub radius: f32,
    pub height: f32,
    pub pose: Mat4,
}

impl ObstacleVolume {
    pub fn of(o: &Obstacle, pose: Mat4) -> Self {
        Self {
            shape: o.shape,
            center: o.center,
            size: o.size,
            radius: o.radius,
            height: o.height,
            pose,
        }
    }

    fn capsule(&self) -> (Vec3, f32, f32) {
        // The pose's axis lengths: the stored pose may be a carve anchor, not a live
        // entity, so it is read off the matrix rather than the world API.
        let size = |axis: glam::Vec4| axis.truncate().length();
        let c = self.pose.transform_point3(self.center);
        let r = self.radius * size(self.pose.x_axis).max(size(self.pose.z_axis));
        (c, r, self.height * size(self.pose.y_axis))
    }

    fn key_points(&self) -> Vec<Vec3> {
        match self.shape {
            ObstacleShape::Box => box_corners(self).to_vec(),
            ObstacleShape::Capsule => {
                let (c, _, h) = self.capsule();
                vec![c - Vec3::Y * h * 0.5, c + Vec3::Y * h * 0.5]
            }
        }
    }

    pub fn displacement(&self, other: &Self) -> f32 {
        let a = self.key_points();
        let b = other.key_points();
        a.iter()
            .zip(&b)
            .map(|(p, q)| p.distance(*q))
            .fold(0.0, f32::max)
    }

    /// An upright prism circumscribing the capsule's footprint for a capsule.
    pub fn triangles(&self) -> Vec<[Vec3; 3]> {
        match self.shape {
            ObstacleShape::Box => {
                let c = box_corners(self);
                BOX_FACES
                    .iter()
                    .flat_map(|f| [[c[f[0]], c[f[1]], c[f[2]]], [c[f[0]], c[f[2]], c[f[3]]]])
                    .collect()
            }
            ObstacleShape::Capsule => self.prism_triangles(),
        }
    }

    fn prism_triangles(&self) -> Vec<[Vec3; 3]> {
        let (c, r, h) = self.capsule();
        let step = std::f32::consts::TAU / ROUND_SEGMENTS as f32;
        let ring_r = r / (step * 0.5).cos();
        let (lo, hi) = (c.y - h * 0.5, c.y + h * 0.5);
        let at = |i: usize, y: f32| {
            let a = step * i as f32;
            Vec3::new(c.x + ring_r * a.cos(), y, c.z + ring_r * a.sin())
        };
        let mut out = Vec::with_capacity(ROUND_SEGMENTS * 4);
        for i in 0..ROUND_SEGMENTS {
            let j = (i + 1) % ROUND_SEGMENTS;
            let (b0, b1, t0, t1) = (at(i, lo), at(j, lo), at(i, hi), at(j, hi));
            out.push([Vec3::new(c.x, lo, c.z), b1, b0]);
            out.push([Vec3::new(c.x, hi, c.z), t0, t1]);
            out.push([b0, b1, t1]);
            out.push([b0, t1, t0]);
        }
        out
    }

    fn avoidance_disc(&self) -> (Vec2, f32) {
        match self.shape {
            ObstacleShape::Capsule => {
                let (c, r, _) = self.capsule();
                (Vec2::new(c.x, c.z), r)
            }
            ObstacleShape::Box => {
                let c = self.pose.transform_point3(self.center);
                let centre = Vec2::new(c.x, c.z);
                let r = box_corners(self)
                    .iter()
                    .map(|p| Vec2::new(p.x, p.z).distance(centre))
                    .fold(0.0, f32::max);
                (centre, r)
            }
        }
    }
}

const BOX_FACES: [[usize; 4]; 6] = [
    [0, 1, 3, 2],
    [4, 6, 7, 5],
    [0, 4, 5, 1],
    [2, 3, 7, 6],
    [0, 2, 6, 4],
    [1, 5, 7, 3],
];

/// A box obstacle's eight world corners; bit 0/1/2 of the index picks +x/+y/+z.
fn box_corners(v: &ObstacleVolume) -> [Vec3; 8] {
    std::array::from_fn(|i| {
        let sign = |bit: usize| if i >> bit & 1 == 1 { 0.5 } else { -0.5 };
        let local = v.center + v.size * Vec3::new(sign(0), sign(1), sign(2));
        v.pose.transform_point3(local)
    })
}

/// An obstacle seen for the first time counts as stationary.
pub fn tick_obstacles(scene: &mut Scene, dt: f32) {
    for id in scene.world.ids_with_nav_obstacle() {
        let pose = scene.world.local_to_world(id);
        let Some(mut o) = scene.world.nav_obstacle_mut(id) else {
            continue;
        };
        let position = pose.w_axis.truncate();
        o.velocity = match o.last_position {
            Some(p) if dt > 0.0 => (position - p) / dt,
            _ => Vec3::ZERO,
        };
        o.last_position = Some(position);
        let now = ObstacleVolume::of(&o, pose);
        match o.carve_pose {
            None => {
                o.carve_pose = Some(pose);
                o.stationary_time = o.time_to_stationary;
            }
            Some(anchor)
                if now.displacement(&ObstacleVolume::of(&o, anchor)) > o.move_threshold =>
            {
                o.carve_pose = Some(pose);
                o.stationary_time = 0.0;
            }
            Some(_) => o.stationary_time += dt,
        }
    }
}

pub(super) fn carving_volumes(scene: &Scene) -> Vec<(u32, ObstacleVolume)> {
    let mut out: Vec<_> = scene
        .world
        .ids_with_nav_obstacle()
        .into_iter()
        .filter(|&id| scene.world.is_active_in_hierarchy(id))
        .filter_map(|id| {
            let o = scene.world.nav_obstacle(id)?;
            o.is_carving().then(|| {
                let pose = o
                    .carve_pose
                    .unwrap_or_else(|| scene.world.local_to_world(id));
                (id, ObstacleVolume::of(&o, pose))
            })
        })
        .collect();
    out.sort_by_key(|&(id, _)| id);
    out
}

/// It never dodges back, like a Unity obstacle.
pub(super) fn avoidance_obstacles(scene: &Scene) -> Vec<AvoidanceAgent> {
    let mut ids = scene.world.ids_with_nav_obstacle();
    ids.sort_unstable();
    ids.into_iter()
        .filter(|&id| scene.world.is_active_in_hierarchy(id))
        .filter_map(|id| {
            let o = scene.world.nav_obstacle(id)?;
            if !o.active || o.is_carving() {
                return None;
            }
            let pose = scene.world.local_to_world(id);
            let (position, radius) = ObstacleVolume::of(&o, pose).avoidance_disc();
            let velocity = Vec2::new(o.velocity.x, o.velocity.z);
            Some(AvoidanceAgent {
                position,
                velocity,
                preferred: velocity,
                radius,
                max_speed: velocity.length(),
                priority: 0,
                solves: false,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "obstacle_tests.rs"]
mod tests;
