//! A variant whose cut moves draws in the dynamic sweep even when static: a cached bake would
//! freeze it. The pipeline cannot tell a move from a cut, so a scroller that clips nothing pays.

use crate::draw::batch::BatchKey;
use crate::draw::uniforms::mapping_uniform;
use crate::gpu::pipelines::surface::{SurfaceShaders, STANDARD};
use crate::gpu::uniforms::MappingUniform;
use crate::scene::MaterialRecords;
use rusty_components::{MaterialAsset, RenderMode};

/// A cutout alone reads its test per caster, never the record, so it carries none: Cutouts sharing
/// a map but not a tint stay one shadow draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Clip {
    pub pipeline: usize,
    pub material: usize,
    pub record: Option<u32>,
}

impl Clip {
    pub(crate) fn by_class(self, records: &MaterialRecords) -> Self {
        let record = self.record.map(|r| records.rep(r));
        Self { record, ..self }
    }

    #[cfg(test)]
    pub(crate) fn of(pipeline: usize, material: usize) -> Self {
        Self {
            pipeline,
            material,
            record: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CasterClip {
    pub clip: Clip,
    pub cutout: Option<Cutout>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cutout {
    pub cutoff: f32,
    pub textured: bool,
    pub mapping: MappingUniform,
}

impl Cutout {
    pub(crate) fn of(asset: &MaterialAsset) -> Option<Self> {
        (asset.render_mode == RenderMode::Cutout).then(|| Self {
            cutoff: asset.alpha_cutoff,
            textured: asset.base_color_map.is_some(),
            mapping: mapping_uniform(asset),
        })
    }
}

pub(crate) fn slot_clip(
    surfaces: &SurfaceShaders,
    key: &BatchKey,
    asset: &MaterialAsset,
) -> Option<CasterClip> {
    let cuts = surfaces.cut(key.pipeline).is_some();
    let cutout = Cutout::of(asset);
    if !cuts && cutout.is_none() {
        return None;
    }
    let clip = Clip {
        pipeline: if cuts { key.pipeline } else { STANDARD },
        record: cuts.then_some(key.record),
        material: key.material,
    };
    Some(CasterClip { clip, cutout })
}

/// The prepass reads the shadow pass's clip, not a rule of its own: a slot wrongly marked would
/// prime depth where the forward pass discards.
pub(crate) fn depth_only(pipeline: usize, clip: Option<&CasterClip>) -> bool {
    pipeline == STANDARD && clip.is_none()
}

pub(crate) fn moves(surfaces: &SurfaceShaders, pipeline: usize) -> bool {
    pipeline != STANDARD && surfaces.cut(pipeline).is_some()
}

#[cfg(test)]
#[path = "clips_tests.rs"]
mod tests;
