use glam::camera::rh::{proj::directx, view::look_to_mat4};
use glam::{Mat4, Vec3};

use crate::clusters::LocalLight;
use crate::Frustum;

pub(crate) const ATLAS_SIZE: u32 = 2048;
pub(crate) const MAX_TILE: u32 = 512;
pub(crate) const MIN_TILE: u32 = 128;
pub(super) const CELLS: u32 = ATLAS_SIZE / MIN_TILE;
pub(crate) const MAX_TILES: usize = (CELLS * CELLS) as usize;
const CUBE_FACES: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::NEG_Y),
    (Vec3::NEG_X, Vec3::NEG_Y),
    (Vec3::Y, Vec3::Z),
    (Vec3::NEG_Y, Vec3::NEG_Z),
    (Vec3::Z, Vec3::NEG_Y),
    (Vec3::NEG_Z, Vec3::NEG_Y),
];
const MAX_SPOT_FOV: f32 = 170.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tile {
    pub view_proj: Mat4,
    pub origin: [u32; 2],
    pub size: u32,
    pub texel: f32,
    pub light: Vec3,
    pub statics: bool,
}

#[derive(Debug, Default)]
pub(crate) struct AtlasPlan {
    pub tiles: Vec<Tile>,
    pub shadows: Vec<(usize, u32)>,
    pub dropped: u32,
    pub resizing: bool,
}

impl AtlasPlan {
    pub(crate) fn texels(&self) -> u64 {
        self.tiles.iter().map(|t| u64::from(t.size).pow(2)).sum()
    }
}

pub(super) struct Candidate {
    pub light: usize,
    pub importance: f32,
    pub size: u32,
    pub want: f32,
}

pub(crate) fn plan(lights: &[(LocalLight, bool)], view_proj: Mat4, eye: Vec3) -> AtlasPlan {
    let ids: Vec<u32> = (0..lights.len() as u32).collect();
    let frame = super::slots::Frame {
        lights,
        ids: &ids,
        view_proj,
        eye,
    };
    super::slots::ShadowSlots::default().plan(None, &frame, |_| true)
}

pub(super) fn candidates(
    lights: &[(LocalLight, bool)],
    view_proj: Mat4,
    eye: Vec3,
) -> Vec<Candidate> {
    let frustum = Frustum::from_view_proj(view_proj);
    let mut candidates: Vec<Candidate> = lights
        .iter()
        .enumerate()
        .filter(|(_, (light, casts))| *casts && light.range > 0.0 && light.intensity > 0.0)
        .filter(|(_, (light, _))| {
            let (center, r) = light.sphere();
            frustum.intersects_aabb(center - Vec3::splat(r), center + Vec3::splat(r))
        })
        .map(|(i, (light, _))| candidate(i, light, eye))
        .collect();
    candidates.sort_by(|a, b| (b.importance.total_cmp(&a.importance)).then(a.light.cmp(&b.light)));
    candidates
}

fn candidate(light: usize, l: &LocalLight, eye: Vec3) -> Candidate {
    let distance = eye.distance(l.sphere().0).max(1e-3);
    let coverage = (l.range / distance).min(1.0);
    let want = MAX_TILE as f32 * coverage;
    let size = prev_power_of_two(want as u32).clamp(MIN_TILE, MAX_TILE);
    Candidate {
        light,
        importance: coverage * l.intensity,
        size,
        want,
    }
}

fn prev_power_of_two(n: u32) -> u32 {
    match n {
        0 => 0,
        n => 1 << (31 - n.leading_zeros()),
    }
}

pub(super) fn faces(light: &LocalLight) -> u32 {
    if light.one_frustum() {
        1
    } else {
        6
    }
}

fn near_plane(light: &LocalLight) -> f32 {
    0.05_f32.min(light.range * 0.5)
}

fn spot_fov(light: &LocalLight) -> f32 {
    (2.0 * light.outer_cone.clamp(-1.0, 1.0).acos()).min(MAX_SPOT_FOV.to_radians())
}

pub(crate) fn light_views(light: &LocalLight) -> Vec<Mat4> {
    let position = Vec3::from(light.position);
    let (near, far) = (near_plane(light), light.range.max(near_plane(light) * 2.0));
    if light.one_frustum() {
        let dir = Vec3::from(light.direction);
        let up = if dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
        let proj = directx::perspective(spot_fov(light), 1.0, near, far);
        return vec![proj * look_to_mat4(position, dir, up)];
    }
    let proj = directx::perspective(std::f32::consts::FRAC_PI_2, 1.0, near, far);
    CUBE_FACES
        .iter()
        .map(|&(dir, up)| proj * look_to_mat4(position, dir, up))
        .collect()
}

pub(super) fn texel_at_one_metre(light: &LocalLight, size: u32) -> f32 {
    let fov = if light.one_frustum() {
        spot_fov(light)
    } else {
        std::f32::consts::FRAC_PI_2
    };
    2.0 * (fov * 0.5).tan() / size as f32
}
