//! Which point and spot lights ask for a shadow this frame, and how big (#468).
//!
//! Pure CPU. A light that wants a shadow and reaches into the view is a
//! candidate. The caller decides "wants": it casts (`cast_shadows`) and the
//! camera's light budget keeps it (`clusters::shadow_requests`, #873), so a light
//! that is never shaded never takes tiles. Candidates are ranked by
//! **importance**, how much of the screen the light can cover times its intensity,
//! the most important first. Each one asks for a tile size from its screen
//! coverage (Unity URP's per-light resolution tier, picked automatically). A
//! spotlight takes one tile and a point light six (a cube, one face per tile).
//!
//! Tiles are square powers of two from [`MIN_TILE`] to [`MAX_TILE`], over a grid of
//! [`MIN_TILE`] cells. Where they go, and how long a light keeps them, is
//! [`super::slots`]'s (#1711): this file only ranks and sizes.

use glam::camera::rh::{proj::directx, view::look_to_mat4};
use glam::{Mat4, Vec3};

use crate::render::clusters::LocalLight;
use crate::render::Frustum;

/// The atlas's side in texels: 2048² is Unity URP's default for additional lights.
pub(crate) const ATLAS_SIZE: u32 = 2048;
/// The largest tile one light face gets.
pub(crate) const MAX_TILE: u32 = 512;
/// The smallest tile, and the packing grid's cell.
pub(crate) const MIN_TILE: u32 = 128;
/// Cells across the packing grid.
pub(super) const CELLS: u32 = ATLAS_SIZE / MIN_TILE;
/// The most tiles the atlas can hold: every cell its own [`MIN_TILE`] tile.
pub(crate) const MAX_TILES: usize = (CELLS * CELLS) as usize;
/// A point light's tiles: its cube faces in the order the shader picks them.
const CUBE_FACES: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::NEG_Y),
    (Vec3::NEG_X, Vec3::NEG_Y),
    (Vec3::Y, Vec3::Z),
    (Vec3::NEG_Y, Vec3::NEG_Z),
    (Vec3::Z, Vec3::NEG_Y),
    (Vec3::NEG_Z, Vec3::NEG_Y),
];
/// The widest spotlight cone the atlas renders; a perspective view stops at 180°.
const MAX_SPOT_FOV: f32 = 170.0;

/// One tile: the light face's view-projection and its square in the atlas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tile {
    pub view_proj: Mat4,
    /// Top-left corner, in texels.
    pub origin: [u32; 2],
    /// Side, in texels.
    pub size: u32,
    /// World size of one texel at one metre from the light (normal-offset bias).
    pub texel: f32,
}

/// The frame's atlas: every tile, and which light owns which.
#[derive(Debug, Default)]
pub(crate) struct AtlasPlan {
    pub tiles: Vec<Tile>,
    /// `(light index, its first tile)` for every light given a shadow.
    pub shadows: Vec<(usize, u32)>,
    /// Lights that asked for a shadow and reach the view, but did not fit.
    pub dropped: u32,
}

impl AtlasPlan {
    /// Texels the frame's tiles cover.
    pub(crate) fn texels(&self) -> u64 {
        self.tiles.iter().map(|t| u64::from(t.size).pow(2)).sum()
    }
}

/// A light that asked for a shadow and reaches the view.
pub(super) struct Candidate {
    /// Its index in the frame's lights.
    pub light: usize,
    pub importance: f32,
    /// The tile side its screen coverage asks for, a power of two.
    pub size: u32,
    /// The same, before rounding to a power of two: what the size hysteresis reads.
    pub want: f32,
}

/// Plan the atlas for `lights` (each with whether it asks for a shadow), seen
/// through `view_proj` from `eye`, with no tile kept from an earlier frame: each
/// light keyed by its index. Bake captures plan this way (#1711), so they never
/// move the live frame's tiles.
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

/// The lights that ask for a shadow and reach the view, the most important first.
/// A light at intensity 0 is no candidate: it shades nothing, so it draws no shadow,
/// though it keeps its tiles while it is off (#1711).
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
    // Most important first; the index breaks ties, so the plan is deterministic.
    candidates.sort_by(|a, b| (b.importance.total_cmp(&a.importance)).then(a.light.cmp(&b.light)));
    candidates
}

/// A shadowed light's rank and the tile size its screen coverage asks for: the
/// range over the distance (1 with the camera inside it), so a light filling the
/// view gets [`MAX_TILE`] and a distant one [`MIN_TILE`].
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

/// The largest power of two `≤ n` (0 for 0).
fn prev_power_of_two(n: u32) -> u32 {
    match n {
        0 => 0,
        n => 1 << (31 - n.leading_zeros()),
    }
}

/// How many tiles a light takes: a spotlight one, a point light its six cube faces.
pub(super) fn faces(light: &LocalLight) -> u32 {
    if light.one_frustum() {
        1
    } else {
        6
    }
}

/// The near plane of a light's shadow views: close enough for a light against a
/// wall, never past its range.
fn near_plane(light: &LocalLight) -> f32 {
    0.05_f32.min(light.range * 0.5)
}

/// A spotlight's full cone angle in radians (its outer cone, both sides).
fn spot_fov(light: &LocalLight) -> f32 {
    (2.0 * light.outer_cone.clamp(-1.0, 1.0).acos()).min(MAX_SPOT_FOV.to_radians())
}

/// The view-projections of a light's tiles: one for a spotlight, six for a point.
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

/// World size of one texel of a `size` tile, one metre from the light.
pub(super) fn texel_at_one_metre(light: &LocalLight, size: u32) -> f32 {
    let fov = if light.one_frustum() {
        spot_fov(light)
    } else {
        std::f32::consts::FRAC_PI_2
    };
    2.0 * (fov * 0.5).tan() / size as f32
}
