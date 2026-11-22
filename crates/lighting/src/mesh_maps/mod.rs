mod curvature;
mod floor;
mod margin;
mod overlap;
mod rays;
mod settings;
mod source;

use glam::{Vec3, Vec4};

use super::lightmap::{world_per_uv, Surface, TexelPoint};
pub use floor::Raised;
pub use overlap::Stacked;
use rays::MeshRays;
use rusty_procgen::image_buf::{Image, Size};
use rusty_procgen::Slot;
pub use settings::{Faces, Settings, MAX_RAYS};
pub use source::MeshSource;

/// In texels: Substance's 8K.
pub const MAX_RESOLUTION: u32 = 8192;

/// Texels filled around each UV island: Substance's default dilation width.
pub const MARGIN: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MeshMap {
    Ao,
    Coverage,
    Curvature,
    Id,
    Position,
    Thickness,
    WorldNormal,
}

pub(super) enum Margin {
    Average,
    Copy,
    None,
}

pub(super) struct Row {
    map: MeshMap,
    name: &'static str,
    pub(super) off: Vec4,
    pub(super) margin: Margin,
    slot: Slot,
}

const GREY: Vec4 = Vec4::new(0.5, 0.5, 0.5, 1.0);

/// One row per map, sorted by name: its value off the islands, how its margin fills, and how its
/// PNG is encoded.
#[rustfmt::skip]
const ROWS: [Row; 7] = [
    row(MeshMap::Ao, "ao", Vec4::ONE, Margin::Average, Slot::Data),
    row(MeshMap::Coverage, "coverage", Vec4::W, Margin::None, Slot::Data),
    row(MeshMap::Curvature, "curvature", GREY, Margin::Average, Slot::Data),
    row(MeshMap::Id, "id", Vec4::W, Margin::Copy, Slot::Normal),
    row(MeshMap::Position, "position", Vec4::W, Margin::Average, Slot::Data),
    row(MeshMap::Thickness, "thickness", Vec4::ONE, Margin::Average, Slot::Data),
    row(MeshMap::WorldNormal, "world_normal", GREY, Margin::Average, Slot::Data),
];

/// How many variants `MeshMap` has. A new variant fails to compile in this `match` until it is
/// named here, and the count beside the names is what `ROWS` is held to.
const VARIANTS: usize = {
    const fn count(m: MeshMap) -> usize {
        match m {
            MeshMap::Ao
            | MeshMap::Coverage
            | MeshMap::Curvature
            | MeshMap::Id
            | MeshMap::Position
            | MeshMap::Thickness
            | MeshMap::WorldNormal => 7,
        }
    }
    count(MeshMap::Ao)
};

const _: () = {
    assert!(ROWS.len() == VARIANTS, "every MeshMap has one row in ROWS");
    let mut i = 0;
    while i < ROWS.len() {
        assert!(
            ROWS[i].map as usize == i,
            "ROWS lists the maps in the enum's order"
        );
        i += 1;
    }
};

const fn row(map: MeshMap, name: &'static str, off: Vec4, margin: Margin, slot: Slot) -> Row {
    Row {
        map,
        name,
        off,
        margin,
        slot,
    }
}

impl MeshMap {
    pub const ALL: [MeshMap; ROWS.len()] = {
        let mut all = [MeshMap::Ao; ROWS.len()];
        let mut i = 0;
        while i < ROWS.len() {
            all[i] = ROWS[i].map;
            i += 1;
        }
        all
    };

    pub(super) fn row(self) -> &'static Row {
        &ROWS[self as usize]
    }

    pub fn name(self) -> &'static str {
        self.row().name
    }

    /// How the map's PNG is encoded.
    pub fn slot(self) -> Slot {
        self.row().slot
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
    settings: &'a Settings,
    /// `source.indices` with every face that writes no texel collapsed to a point.
    written: Option<Vec<u32>>,
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
                let t: &MeshRays =
                    tracer.get_or_insert_with(|| MeshRays::new(source, &settings.faces));
                match map {
                    MeshMap::Ao => image(&|i, p| t.ao(&baked, i, p)),
                    _ => image(&|i, p| t.thickness(&baked, i, p)),
                }
            }
            MeshMap::WorldNormal => image(&|_, p| (p.normal * 0.5 + 0.5).extend(1.0)),
            MeshMap::Position => image(&|_, p| baked.normalised(p.position).extend(1.0)),
            MeshMap::Id => image(&|_, p| margin::id_colour(source.material_key(p.tri()))),
            MeshMap::Coverage => image(&|_, _| Vec4::ONE),
        };
        sink(map, image)?;
    }
    Ok(Notes { raised, stacked })
}

impl<'a> Baked<'a> {
    fn new(
        source: &'a MeshSource,
        settings: &'a Settings,
    ) -> Result<(Self, Option<MeshRays>, Option<Stacked>), String> {
        settings.check()?;
        source.check()?;
        let (min, max) = source.positions.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), &p| (lo.min(p), hi.max(p)),
        );
        let diagonal = (max - min).length();
        settings.check_reach(diagonal)?;
        let faces = &settings.faces;
        let written = (!faces.every()).then(|| {
            let tris = source.indices.chunks_exact(3).enumerate();
            tris.flat_map(|(k, t)| match faces.writes(source.material(3 * k)) {
                true => [t[0], t[1], t[2]],
                false => [t[0]; 3],
            })
            .collect()
        });
        let mut baked = Baked {
            source,
            settings,
            written,
            points: Vec::new(),
            min,
            extent: max - min,
            diagonal,
        };
        let mut rays = None;
        let (points, stacked) = overlap::points(&baked, &mut rays);
        if points.is_empty() {
            return Err(match settings.faces.materials {
                Some(_) => "has no triangle of the asked materials inside UV0's 0..1 square",
                None => "has no triangle inside UV0's 0..1 square",
            }
            .into());
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

    fn written(&self) -> Surface<'_> {
        let mut surface = self.source.surface();
        if let Some(indices) = &self.written {
            surface.indices = indices;
        }
        surface
    }

    fn texel_world(&self) -> f32 {
        self.texel_world_of(world_per_uv(&self.written()).unwrap_or(self.diagonal))
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
mod coverage_tests;
#[cfg(test)]
mod edge_tests;
#[cfg(test)]
mod faces_tests;
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
