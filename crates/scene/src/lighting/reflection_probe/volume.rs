use glam::{Quat, Vec3};

use super::{finite_or, ProbeShape, ReflectionProbe, MIN_RADIUS};

/// The shader's frame: a world point turned by `inverse` about the origin, where the box is
/// axis-aligned. Unrotated, it is the world, bit for bit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeFrame {
    pub inverse: Quat,
    pub center: Vec3,
    pub box_min: Vec3,
    pub box_max: Vec3,
    pub sphere: Option<f32>,
    pub influence: f32,
    pub fade: f32,
}

impl ProbeFrame {
    pub fn to_frame(&self, v: Vec3) -> Vec3 {
        rotate(self.inverse, v)
    }

    pub fn to_world(&self, v: Vec3) -> Vec3 {
        rotate(self.inverse.conjugate(), v)
    }

    /// Past the influence, the span the projection fades out over: the half-diagonal (the
    /// radius), scale-free, so a 2 m box and a 40 m hall fade alike.
    pub fn fade_span(&self) -> f32 {
        let size = match self.sphere {
            Some(r) => r,
            None => (self.box_max - self.box_min).length() * 0.5,
        };
        finite_or(size, MIN_RADIUS).max(MIN_RADIUS)
    }

    pub fn distance_sq(&self, point: Vec3) -> f32 {
        self.local_distance_sq(self.to_frame(point))
    }

    pub(super) fn local_distance_sq(&self, local: Vec3) -> f32 {
        self.nearest(local).distance_squared(local)
    }

    pub(super) fn outside(&self, point: Vec3) -> f32 {
        self.nearest(point).distance(point)
    }

    fn nearest(&self, point: Vec3) -> Vec3 {
        match self.sphere {
            None => point.clamp(self.box_min, self.box_max),
            Some(r) => {
                let off = point - self.center;
                match off.length() > r {
                    true => self.center + off * (r / off.length()),
                    false => point,
                }
            }
        }
    }
}

/// The shader's `quat_rotate`, step for step: an identity adds an exact zero, changing no bits.
pub(crate) fn rotate(q: Quat, v: Vec3) -> Vec3 {
    let u = Vec3::new(q.x, q.y, q.z);
    v + 2.0 * u.cross(u.cross(v) + q.w * v)
}

impl ReflectionProbe {
    pub fn frame(&self) -> ProbeFrame {
        let sphere = (self.shape == ProbeShape::Sphere).then_some(self.radius);
        let mut frame = ProbeFrame {
            inverse: Quat::IDENTITY,
            center: self.position,
            box_min: self.box_min,
            box_max: self.box_max,
            sphere,
            influence: self.blend_distance.max(0.0),
            fade: 0.0,
        };
        if self.rotation != Quat::IDENTITY {
            frame.inverse = self.rotation.conjugate();
            frame.center = rotate(frame.inverse, self.position);
            let shift = frame.center - self.position;
            (frame.box_min, frame.box_max) = (self.box_min + shift, self.box_max + shift);
        }
        frame.fade = frame.fade_span();
        frame
    }

    pub fn contains(&self, point: Vec3) -> bool {
        self.volume_distance_sq(point) <= 0.0
    }

    pub fn volume_distance_sq(&self, point: Vec3) -> f32 {
        self.frame().distance_sq(point)
    }

    pub(super) fn distance_sq(&self, point: Vec3) -> f32 {
        self.position.distance_squared(point)
    }

    /// Bit 0 of the index picks max x, bit 1 max y, bit 2 max z.
    pub fn corners(&self) -> [Vec3; 8] {
        let (lo, hi) = (self.box_min, self.box_max);
        std::array::from_fn(|i| {
            let pick = |bit: usize, a: f32, b: f32| if i & bit == 0 { a } else { b };
            let c = Vec3::new(
                pick(1, lo.x, hi.x),
                pick(2, lo.y, hi.y),
                pick(4, lo.z, hi.z),
            );
            match self.rotation == Quat::IDENTITY {
                true => c,
                false => self.position + self.rotation * (c - self.position),
            }
        })
    }

    pub fn world_bounds(&self) -> (Vec3, Vec3) {
        if self.shape == ProbeShape::Sphere {
            let r = Vec3::splat(self.radius);
            return (self.position - r, self.position + r);
        }
        let corners = self.corners();
        let lo = corners.iter().fold(Vec3::INFINITY, |a, &c| a.min(c));
        let hi = corners.iter().fold(Vec3::NEG_INFINITY, |a, &c| a.max(c));
        (lo, hi)
    }
}

/// Points a side at which `share_of` samples a box an oriented volume covers.
const SHARE_SAMPLES: usize = 16;

impl ReflectionProbe {
    pub fn overlaps(&self, other: &Self) -> bool {
        match (self.shape, other.shape) {
            (ProbeShape::Sphere, _) => {
                other.volume_distance_sq(self.position) < self.radius.powi(2)
            }
            (_, ProbeShape::Sphere) => {
                self.volume_distance_sq(other.position) < other.radius.powi(2)
            }
            _ => boxes_overlap(&self.oriented(), &other.oriented()),
        }
    }

    pub fn share_of(&self, lo: Vec3, hi: Vec3) -> f32 {
        let volume = |a: Vec3, b: Vec3| (b - a).max(Vec3::ZERO).element_product();
        let own = volume(lo, hi);
        if own <= 0.0 {
            return 0.0;
        }
        if self.shape == ProbeShape::Box && self.rotation == Quat::IDENTITY {
            return volume(self.box_min.max(lo), self.box_max.min(hi)) / own;
        }
        let n = SHARE_SAMPLES;
        let cell = (hi - lo) / n as f32;
        let at = |i: usize| i as f32 + 0.5;
        let inside = (0..n * n * n)
            .filter(|&k| {
                let idx = Vec3::new(at(k % n), at(k / n % n), at(k / (n * n)));
                self.contains(lo + cell * idx)
            })
            .count();
        inside as f32 / (n * n * n) as f32
    }

    fn oriented(&self) -> (Vec3, [Vec3; 3], Vec3) {
        let mid = (self.box_min + self.box_max) * 0.5;
        let centre = self.position + self.rotation * (mid - self.position);
        let axes = [Vec3::X, Vec3::Y, Vec3::Z].map(|a| self.rotation * a);
        (centre, axes, (self.box_max - self.box_min).abs() * 0.5)
    }
}

fn boxes_overlap(a: &(Vec3, [Vec3; 3], Vec3), b: &(Vec3, [Vec3; 3], Vec3)) -> bool {
    let reach = |(_, axes, half): &(Vec3, [Vec3; 3], Vec3), l: Vec3| {
        (0..3).map(|i| half[i] * axes[i].dot(l).abs()).sum::<f32>()
    };
    let gap = b.0 - a.0;
    let separates =
        |l: Vec3| l.length_squared() > 1e-10 && gap.dot(l).abs() >= reach(a, l) + reach(b, l);
    let crossed = (0..9).map(|k| a.1[k / 3].cross(b.1[k % 3]));
    !(a.1.into_iter().chain(b.1).chain(crossed)).any(separates)
}
