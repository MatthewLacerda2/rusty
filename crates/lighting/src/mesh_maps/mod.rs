mod curvature;
mod floor;
mod margin;
mod overlap;
mod rays;
mod settings;
mod source;

use glam::{Vec3, Vec4};

use super::lightmap::{world_per_uv, TexelPoint};
pub use floor::Raised;
pub use overlap::Stacked;
use rays::MeshRays;
use rusty_procgen::image_buf::{Image, Size};
pub use settings::{Settings, MAX_RAYS};
pub use source::MeshSource;

/// In texels: Substance's 8K.
pub const MAX_RESOLUTION: u32 = 8192;

/// Texels filled around each UV island: Substance's default dilation width.
pub const MARGIN: u32 = 16;

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
    pub const ALL: [MeshMap; 6] = [
        MeshMap::Curvature,
        MeshMap::Ao,
        MeshMap::WorldNormal,
        MeshMap::Position,
        MeshMap::Thickness,
        MeshMap::Id,
    ];

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

struct Baked<'a> {
    source: &'a MeshSource,
    settings: Settings,
    points: Vec<TexelPoint>,
    min: Vec3,
    extent: Vec3,
    diagonal: f32,
}

#[derive(Debug, Default, PartialEq)]
pub struct Notes {
    pub raised: Option<Raised>,
    pub stacked: Option<Stacked>,
}

pub fn bake(
    source: &MeshSource,
    maps: &[MeshMap],
    settings: &Settings,
    mut sink: impl FnMut(MeshMap, Image) -> Result<(), String>,
) -> Result<Notes, String> {
    let (baked, mut tracer, stacked) = Baked::new(source, settings)?;
    if !maps
        .iter()
        .any(|m| matches!(m, MeshMap::Ao | MeshMap::Thickness))
    {
        tracer = None;
    }
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
    Ok(Notes { raised, stacked })
}

impl<'a> Baked<'a> {
    fn new(
        source: &'a MeshSource,
        settings: &Settings,
    ) -> Result<(Self, Option<MeshRays>, Option<Stacked>), String> {
        settings.check()?;
        source.check()?;
        let (min, max) = source.positions.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), &p| (lo.min(p), hi.max(p)),
        );
        let diagonal = (max - min).length();
        settings.check_reach(diagonal)?;
        let mut baked = Baked {
            source,
            settings: *settings,
            points: Vec::new(),
            min,
            extent: max - min,
            diagonal,
        };
        let mut rays = None;
        let (points, stacked) = overlap::points(&baked, &mut rays);
        if points.is_empty() {
            return Err("has no triangle inside UV0's 0..1 square".into());
        }
        baked.points = points;
        Ok((baked, rays, stacked))
    }

    fn normalised(&self, p: Vec3) -> Vec3 {
        let span = |d: f32, e: f32| if e > 0.0 { d / e } else { 0.5 };
        let d = p - self.min;
        Vec3::new(
            span(d.x, self.extent.x),
            span(d.y, self.extent.y),
            span(d.z, self.extent.z),
        )
    }

    fn texel_world(&self) -> f32 {
        self.texel_world_of(world_per_uv(&self.source.surface()).unwrap_or(self.diagonal))
    }

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
mod stack_tests;
#[cfg(test)]
mod surface_tests;
#[cfg(test)]
mod tests;
