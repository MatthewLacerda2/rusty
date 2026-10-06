//! Which point and spot lights get a shadow this frame, and where (#468).
//!
//! Pure CPU: the frame's local lights and the base camera in, the atlas tiles out.
//! A light that wants a shadow and reaches into the view is a candidate. The
//! caller decides "wants": it casts (`cast_shadows`) and the camera's light budget
//! keeps it (`clusters::shadow_requests`, #873), so a light that is never shaded
//! never takes tiles. Candidates are ranked by **importance**, how much of the screen the
//! light can cover times its intensity, the most important first. Each one asks
//! for a tile size from its screen coverage (Unity URP's per-light resolution tier,
//! picked automatically), never larger than the tile of a more important light.
//! A spotlight takes one tile and a point light six (a cube, one face per tile).
//!
//! Tiles are square powers of two from [`MIN_TILE`] to [`MAX_TILE`], packed in
//! Z-order over a grid of [`MIN_TILE`] cells. Sizes never grow down the list, so
//! each tile starts aligned to its own size and none overlap. A light that no
//! longer fits is retried at half the size, down to [`MIN_TILE`]. One that still
//! does not fit is **dropped**: it shades unshadowed and is counted in
//! `RenderCounters::shadow_lights_dropped`, never silently.

use glam::camera::rh::{proj::directx, view::look_to_mat4};
use glam::{Mat4, Vec3};

use crate::render::clusters::{LocalLight, KIND_SPOT};
use crate::render::Frustum;

/// The atlas's side in texels: 2048² is Unity URP's default for additional lights.
pub(crate) const ATLAS_SIZE: u32 = 2048;
/// The largest tile one light face gets.
pub(crate) const MAX_TILE: u32 = 512;
/// The smallest tile, and the packing grid's cell.
pub(crate) const MIN_TILE: u32 = 128;
/// Cells across the packing grid.
const CELLS: u32 = ATLAS_SIZE / MIN_TILE;
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
struct Candidate {
    light: usize,
    importance: f32,
    size: u32,
}

/// Plan the atlas for `lights` (each with whether it asks for a shadow), seen
/// through `view_proj` from `eye`.
pub(crate) fn plan(lights: &[(LocalLight, bool)], view_proj: Mat4, eye: Vec3) -> AtlasPlan {
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

    let mut out = AtlasPlan::default();
    let mut cursor = 0;
    let mut cap = MAX_TILE;
    for c in candidates {
        let light = &lights[c.light].0;
        let faces = if light.kind == KIND_SPOT { 1 } else { 6 };
        let mut size = c.size.min(cap);
        while size > MIN_TILE && !fits(cursor, size, faces) {
            size /= 2;
        }
        if !fits(cursor, size, faces) {
            out.dropped += 1;
            continue;
        }
        cap = size;
        out.shadows.push((c.light, out.tiles.len() as u32));
        for view_proj in light_views(light) {
            let texel = texel_at_one_metre(light, size);
            let origin = cell_origin(cursor);
            out.tiles.push(Tile {
                view_proj,
                origin,
                size,
                texel,
            });
            cursor += cells(size);
        }
    }
    out
}

/// A shadowed light's rank and the tile size its screen coverage asks for: the
/// range over the distance (1 with the camera inside it), so a light filling the
/// view gets [`MAX_TILE`] and a distant one [`MIN_TILE`].
fn candidate(light: usize, l: &LocalLight, eye: Vec3) -> Candidate {
    let distance = eye.distance(l.sphere().0).max(1e-3);
    let coverage = (l.range / distance).min(1.0);
    let wanted = (MAX_TILE as f32 * coverage) as u32;
    let size = prev_power_of_two(wanted).clamp(MIN_TILE, MAX_TILE);
    Candidate {
        light,
        importance: coverage * l.intensity,
        size,
    }
}

/// The largest power of two `≤ n` (0 for 0).
fn prev_power_of_two(n: u32) -> u32 {
    match n {
        0 => 0,
        n => 1 << (31 - n.leading_zeros()),
    }
}

/// Grid cells one tile of `size` covers.
fn cells(size: u32) -> u32 {
    (size / MIN_TILE).pow(2)
}

/// Whether `faces` tiles of `size` fit after the first `cursor` cells.
fn fits(cursor: u32, size: u32, faces: u32) -> bool {
    cursor + faces * cells(size) <= CELLS * CELLS
}

/// The texel corner of Z-order cell `index`: its even bits are x, its odd bits y.
fn cell_origin(index: u32) -> [u32; 2] {
    let compact = |mut v: u32| {
        let mut out = 0;
        for bit in 0..16 {
            out |= (v & 1) << bit;
            v >>= 2;
        }
        out
    };
    [compact(index) * MIN_TILE, compact(index >> 1) * MIN_TILE]
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
    if light.kind == KIND_SPOT {
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
fn texel_at_one_metre(light: &LocalLight, size: u32) -> f32 {
    let fov = match light.kind {
        KIND_SPOT => spot_fov(light),
        _ => std::f32::consts::FRAC_PI_2,
    };
    2.0 * (fov * 0.5).tan() / size as f32
}
