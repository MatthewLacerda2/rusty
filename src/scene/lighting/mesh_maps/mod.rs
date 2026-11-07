//! src/scene/lighting/mesh_maps/ — mesh-map bakers (#1849), Substance's *Bake Mesh
//! Maps*: data about one model's surface, written in its UV0 so a texture recipe
//! can read it (`Texture.BakeMeshMaps`, the `image` op). Edge wear follows the
//! model's real edges, grime its real cavities, dust its up-facing faces.
//!
//! Every map walks the model the way the lightmap bake walks a lightmap
//! (`lightmap::rasterize_surface`): each texel a triangle covers becomes a point on
//! the mesh, inset from the triangle's edges, and the empty texels around a UV
//! island are filled [`MARGIN`] texels deep so a filtered or mipped read never
//! seams. Rays go through the lightmapper's `Rays`. Per map:
//!
//! * [`MeshMap::Curvature`] — `curvature`: 0.5 flat, brighter towards a convex
//!   edge, darker towards a concave one;
//! * [`MeshMap::Ao`] — the share of cosine rays that leave the mesh (`rays`);
//! * [`MeshMap::WorldNormal`] — the normal, `n * 0.5 + 0.5` per axis;
//! * [`MeshMap::Position`] — the point, normalised per axis to the mesh's bounds;
//! * [`MeshMap::Thickness`] — how far rays cast into the mesh travel (`rays`);
//! * [`MeshMap::Id`] — one colour per material slot (`margin::id_colour`).
//!
//! The mesh is baked in its own space, as imported: "world" is the model's frame
//! before any scene transform, as Substance bakes an exported mesh. A texel two
//! triangles share (overlapping UVs) takes the first in index order, as the
//! lightmap walk gives it; a triangle outside UV 0..1 bakes only its part inside.
//! The bake is a pure function of (mesh, maps, resolution): each texel's rays draw
//! from their own seeded stream, so threads never show in the bytes.

mod curvature;
mod margin;
mod rays;

use glam::{Vec2, Vec3, Vec4};

use super::lightmap::{rasterize_surface, world_per_uv, Surface, TexelPoint};
use crate::asset::SubMesh;
use crate::procgen::image_buf::Image;

/// The largest map edge, in texels: Substance's 8K.
pub const MAX_RESOLUTION: u32 = 8192;

/// How many texels around each UV island are filled from it: Substance's default
/// dilation width.
pub const MARGIN: u32 = 16;

/// One kind of mesh map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MeshMap {
    Curvature,
    Ao,
    WorldNormal,
    Position,
    Thickness,
    Id,
}

impl MeshMap {
    /// Every map, in the order the docs list them.
    pub const ALL: [MeshMap; 6] = [
        MeshMap::Curvature,
        MeshMap::Ao,
        MeshMap::WorldNormal,
        MeshMap::Position,
        MeshMap::Thickness,
        MeshMap::Id,
    ];

    /// The map's name, as a script writes it and as its file is suffixed.
    pub fn name(self) -> &'static str {
        match self {
            MeshMap::Curvature => "curvature",
            MeshMap::Ao => "ao",
            MeshMap::WorldNormal => "world_normal",
            MeshMap::Position => "position",
            MeshMap::Thickness => "thickness",
            MeshMap::Id => "id",
        }
    }

    /// The map `name` names (case-insensitive); an unknown one is an error that
    /// lists the known.
    pub fn parse(name: &str) -> Result<MeshMap, String> {
        let wanted = name.to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|m| m.name() == wanted)
            .ok_or_else(|| {
                let known: Vec<_> = Self::ALL.iter().map(|m| m.name()).collect();
                format!("unknown mesh map \"{name}\" (known: {})", known.join(", "))
            })
    }
}

/// The mesh a bake reads: unit normals, UV0, the triangles, and where each
/// material slot's run of `indices` ends.
#[derive(Clone, Debug, Default)]
pub struct MeshSource {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub indices: Vec<u32>,
    pub run_ends: Vec<u32>,
}

impl MeshSource {
    /// An imported sub-mesh, in its own space.
    pub fn from_sub_mesh(mesh: &SubMesh) -> Self {
        let v = &mesh.vertices;
        Self {
            positions: v.iter().map(|v| Vec3::from(v.position)).collect(),
            normals: v
                .iter()
                .map(|v| Vec3::from(v.normal).try_normalize().unwrap_or(Vec3::Y))
                .collect(),
            uvs: v.iter().map(|v| Vec2::from(v.tex_coords)).collect(),
            indices: mesh.indices.clone(),
            run_ends: mesh.ranges.iter().map(|r| r.indices.end).collect(),
        }
    }

    fn surface(&self) -> Surface<'_> {
        Surface {
            positions: &self.positions,
            normals: &self.normals,
            uvs: &self.uvs,
            indices: &self.indices,
        }
    }

    /// The material slot the triangle starting at index `first` belongs to.
    fn slot_at(&self, first: usize) -> usize {
        self.run_ends.partition_point(|&end| end as usize <= first)
    }

    /// Why this mesh cannot be baked, if it can't: no triangles, an index past its
    /// vertices, a non-finite position or UV, or UVs that enclose no area.
    fn check(&self) -> Result<(), String> {
        let n = self.positions.len();
        if self.indices.len() < 3 {
            return Err("has no triangles".into());
        }
        if self.normals.len() != n || self.uvs.len() != n {
            return Err("has a vertex without a normal or a UV".into());
        }
        if self.indices.iter().any(|&i| i as usize >= n) {
            return Err("has an index past its vertices".into());
        }
        if !self.positions.iter().all(|p| p.is_finite()) || !self.uvs.iter().all(|u| u.is_finite())
        {
            return Err("has a non-finite position or UV".into());
        }
        world_per_uv(&self.surface())
            .map(|_| ())
            .ok_or_else(|| "has no UV0 (its texture UVs enclose no area)".into())
    }
}

/// What every map of one bake shares: the covered texels and the mesh's bounds.
struct Baked<'a> {
    source: &'a MeshSource,
    size: u32,
    points: Vec<TexelPoint>,
    min: Vec3,
    extent: Vec3,
    /// The bounds' diagonal: the scale every distance in the bake is relative to.
    diagonal: f32,
}

/// Bake `maps` of `source` at `size`² texels, each map once, in the order asked.
/// An error says what is wrong with the mesh or the arguments; the caller names the
/// model.
pub fn bake(
    source: &MeshSource,
    maps: &[MeshMap],
    size: u32,
) -> Result<Vec<(MeshMap, Image)>, String> {
    if !(1..=MAX_RESOLUTION).contains(&size) {
        return Err(format!("resolution {size} is outside 1..={MAX_RESOLUTION}"));
    }
    source.check()?;
    let points = rasterize_surface(&source.surface(), size);
    if points.is_empty() {
        return Err("has no triangle inside UV0's 0..1 square".into());
    }
    let (min, max) = source.positions.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(lo, hi), &p| (lo.min(p), hi.max(p)),
    );
    let baked = Baked {
        source,
        size,
        points,
        min,
        extent: max - min,
        diagonal: (max - min).length(),
    };
    let tracer = maps
        .iter()
        .any(|m| matches!(m, MeshMap::Ao | MeshMap::Thickness))
        .then(|| rays::MeshRays::new(source));
    let mut out: Vec<(MeshMap, Image)> = Vec::new();
    for &map in maps {
        if out.iter().any(|(m, _)| *m == map) {
            continue;
        }
        let values = match map {
            MeshMap::Curvature => curvature::bake(&baked),
            MeshMap::Ao => tracer.as_ref().map(|t| t.ao(&baked)).unwrap_or_default(),
            MeshMap::Thickness => tracer
                .as_ref()
                .map(|t| t.thickness(&baked))
                .unwrap_or_default(),
            MeshMap::WorldNormal => baked.each(|p| (p.normal * 0.5 + 0.5).extend(1.0)),
            MeshMap::Position => baked.each(|p| baked.normalised(p.position).extend(1.0)),
            MeshMap::Id => baked.each(|p| margin::id_colour(source.slot_at(p.first))),
        };
        out.push((map, margin::image(map, &baked, &values)));
    }
    Ok(out)
}

impl Baked<'_> {
    /// `f` of every covered texel, in point order.
    fn each(&self, f: impl Fn(&TexelPoint) -> Vec4 + Sync) -> Vec<Vec4> {
        per_texel(&self.points, |_, p| f(p))
    }

    /// `p` in the bounds, 0..1 per axis; a flat axis reads 0.5.
    fn normalised(&self, p: Vec3) -> Vec3 {
        let span = |d: f32, e: f32| if e > 0.0 { d / e } else { 0.5 };
        let d = p - self.min;
        Vec3::new(
            span(d.x, self.extent.x),
            span(d.y, self.extent.y),
            span(d.z, self.extent.z),
        )
    }

    /// World units one texel spans, on average over the mesh.
    fn texel_world(&self) -> f32 {
        world_per_uv(&self.source.surface()).unwrap_or(self.diagonal) / self.size as f32
    }
}

/// `f(i, point)` for every point, split across the machine's threads in contiguous
/// chunks and joined back in order: which thread ran a texel never shows.
fn per_texel(points: &[TexelPoint], f: impl Fn(usize, &TexelPoint) -> Vec4 + Sync) -> Vec<Vec4> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = points.len().div_ceil(threads).max(1);
    let f = &f;
    std::thread::scope(|s| {
        let handles: Vec<_> = points
            .chunks(chunk)
            .enumerate()
            .map(|(c, part)| {
                s.spawn(move || {
                    let base = c * chunk;
                    part.iter()
                        .enumerate()
                        .map(|(k, p)| f(base + k, p))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a mesh-map worker panicked"))
            .collect()
    })
}

#[cfg(test)]
mod edge_tests;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;
