//! crates/lighting/src/mesh_maps/ — mesh-map bakers (#1849), Substance's *Bake Mesh
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
//! before any scene transform, as Substance bakes an exported mesh. A texel
//! triangles share (overlapping UVs: a panel on a wall, a box's hidden back) takes
//! the most open of them for every map, ties to the lower index (`overlap`, #2310);
//! a triangle outside UV 0..1 bakes only its part inside.
//! [`Settings`] holds what a bake can be told (#2260): its size (square or not),
//! the curvature band's reach and each ray-traced map's ray count. A band is never
//! narrower than three of its UV island's own texels (`floor`, #2292); `bake`
//! returns the islands that floor widened past the radius asked for.
//!
//! The bake is a pure function of (mesh, maps, settings): each texel's rays draw
//! from their own seeded stream, so threads never show in the bytes.
//!
//! Memory: the covered texels' points (32 bytes each) live through the whole bake,
//! and one map at a time beside them: its `Image` (16 bytes a texel), a flag per
//! texel, and the PNG encoder's copy. Measured with all six maps on a quad that
//! covers every texel (the worst case) through `Texture.BakeMeshMaps`: 1.2 GiB peak
//! at 4096², 4.6 GiB at 8192² (2026-11-07, with 40-byte points; at 32 bytes the
//! points take 128 MiB and 512 MiB less). A UV layout with gutters covers less; a
//! non-square map holds its own `width × height` texels. More rays cost time, not
//! memory. Before the maps, the walk holds a 4-byte owner per texel, and a model
//! whose UVs overlap holds every claimant of each shared texel, its owner
//! included (40 bytes each with its place and openness), until `overlap` has
//! picked: both are gone before the first map's image is made.

mod curvature;
mod floor;
mod margin;
mod overlap;
mod rays;
mod settings;
mod source;

use glam::{Vec3, Vec4};

use super::lightmap::{rasterize_overlaps, world_per_uv, TexelPoint};
pub use floor::Raised;
use rays::MeshRays;
use rusty_procgen::image_buf::{Image, Size};
pub use settings::{Settings, MAX_RAYS};
pub use source::MeshSource;

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

/// What every map of one bake shares: the covered texels and the mesh's bounds.
struct Baked<'a> {
    source: &'a MeshSource,
    settings: Settings,
    points: Vec<TexelPoint>,
    min: Vec3,
    extent: Vec3,
    /// The bounds' diagonal: the scale every distance in the bake is relative to.
    diagonal: f32,
}

/// Bake `maps` of `source` as `settings` says, each map once, in the order asked,
/// handing each to `sink` as it is done: one map's image is alive at a time (its
/// peak memory is in the module docs). Returns the UV islands whose texels were
/// too coarse for the `curvature_radius` named, which baked wider (`floor`), for
/// the caller to tell. An error says what is wrong with the mesh or the arguments,
/// or is the first one `sink` returned (no later map is baked); the caller names
/// the model.
pub fn bake(
    source: &MeshSource,
    maps: &[MeshMap],
    settings: &Settings,
    mut sink: impl FnMut(MeshMap, Image) -> Result<(), String>,
) -> Result<Option<Raised>, String> {
    let (baked, mut tracer) = Baked::new(source, settings)?;
    let mut raised = None;
    let mut done = Vec::new();
    for &map in maps {
        if done.contains(&map) {
            continue;
        }
        done.push(map);
        let image = |f: &(dyn Fn(usize, &TexelPoint) -> Vec4 + Sync)| margin::image(map, &baked, f);
        let image = match map {
            MeshMap::Curvature => {
                let (c, floor) = curvature::Curvature::new(&baked);
                raised = floor;
                image(&|_, p| c.texel(p))
            }
            MeshMap::Ao | MeshMap::Thickness => {
                let t: &MeshRays = tracer.get_or_insert_with(|| MeshRays::new(source));
                match map {
                    MeshMap::Ao => image(&|i, p| t.ao(&baked, i, p)),
                    _ => image(&|i, p| t.thickness(&baked, i, p)),
                }
            }
            MeshMap::WorldNormal => image(&|_, p| (p.normal * 0.5 + 0.5).extend(1.0)),
            MeshMap::Position => image(&|_, p| baked.normalised(p.position).extend(1.0)),
            MeshMap::Id => image(&|_, p| margin::id_colour(source.material_key(p.tri()))),
        };
        sink(map, image)?;
    }
    Ok(raised)
}

impl<'a> Baked<'a> {
    /// The covered texels and bounds of `source` at `settings`, or why they can't
    /// be baked (the settings, the mesh, or no triangle inside UV0). A texel
    /// triangles share in UV0 holds its most open one's point (`overlap`); the
    /// model's tracer comes back when that took one, for the ray-traced maps.
    fn new(
        source: &'a MeshSource,
        settings: &Settings,
    ) -> Result<(Self, Option<MeshRays>), String> {
        settings.check()?;
        source.check()?;
        let Size { width, height } = settings.size;
        let (points, others) = rasterize_overlaps(&source.surface(), width, height);
        if points.is_empty() {
            return Err("has no triangle inside UV0's 0..1 square".into());
        }
        let (min, max) = source.positions.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), &p| (lo.min(p), hi.max(p)),
        );
        let diagonal = (max - min).length();
        settings.check_reach(diagonal)?;
        let mut baked = Baked {
            source,
            settings: *settings,
            points,
            min,
            extent: max - min,
            diagonal,
        };
        if others.is_empty() {
            return Ok((baked, None));
        }
        let rays = MeshRays::new(source);
        overlap::resolve(&mut baked, others, &rays);
        Ok((baked, Some(rays)))
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

    /// World units one texel spans, on average over the mesh: the root of a
    /// texel's world area, so a non-square map whose model is as long as its UV is
    /// stretched (a strip light at 4096×64) measures its real, square texel. A
    /// square map divides by its edge exactly (the product is taken in `f64`).
    fn texel_world(&self) -> f32 {
        self.texel_world_of(world_per_uv(&self.source.surface()).unwrap_or(self.diagonal))
    }

    /// World units one texel spans where one unit of UV spans `per_uv` (a UV
    /// island's measure, or the mesh's in [`Baked::texel_world`]).
    fn texel_world_of(&self, per_uv: f32) -> f32 {
        let Size { width, height } = self.settings.size;
        let texels = (f64::from(width) * f64::from(height)).sqrt() as f32;
        per_uv / texels
    }
}

#[cfg(test)]
mod bevel_tests;
#[cfg(test)]
mod edge_tests;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod floor_tests;
#[cfg(test)]
mod overlap_tests;
#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod surface_tests;
#[cfg(test)]
mod tests;
