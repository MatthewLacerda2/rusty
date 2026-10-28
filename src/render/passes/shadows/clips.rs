//! Which shadow casters are clipped, and by what (#648).
//!
//! The plain depth pipeline has no fragment stage: every triangle of a caster lands in
//! the cascade. Two kinds of caster need their fragments tested instead, and are drawn
//! through a clipping pipeline that binds their material's group 2:
//!
//! - a **Cutout** material (#242): its albedo's alpha below the cutoff casts nothing,
//!   as it shows nothing — `shader.wgsl`'s `fs_shadow`;
//! - a **surface variant whose blocks cut or move the UVs** (`dissolve`, #400;
//!   `uv_scroll`, #1499): its `fs_shadow_cut` runs the same cut as its colour pass,
//!   against the same runtime params and mask, and clips Cutout texels at the UVs
//!   the colour pass samples.
//!
//! Such a variant's cut moves at runtime (a script drives `dissolve.amount`, the game
//! time scrolls the UVs), so its caster is drawn in the dynamic sweep even when
//! static: the static bake is cached until its light volume moves, and would keep a
//! stale cut. A scrolling variant that is not Cutout pays for that sweep without
//! clipping anything: the pipeline cannot tell a move from a cut.

use std::collections::HashMap;

use crate::components::RenderMode;
use crate::render::draw::materials::entity_params;
use crate::render::gpu::pipelines::surface::STANDARD;
use crate::render::Renderer;
use crate::scene::Scene;

/// A clipped caster's draw key: the surface pipeline whose cut it runs
/// ([`STANDARD`] for a cutout alone) and its material group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Clip {
    pub pipeline: usize,
    pub material: usize,
}

impl Clip {
    /// Whether the cut is a surface variant's, which moves at runtime.
    pub(crate) fn is_variant(self) -> bool {
        self.pipeline != STANDARD
    }
}

/// How one caster's shadow is clipped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CasterClip {
    pub clip: Clip,
    /// The Cutout material's alpha cutoff, and whether it has an albedo map to test.
    pub cutout: Option<(f32, bool)>,
}

impl Renderer {
    /// Every active mesh entity's clipped material slots, by `(id, slot)` (#1285) —
    /// resolving each slot's surface pipeline and material group exactly as its
    /// forward draw does, so both passes read one param buffer and mask.
    pub(super) fn shadow_clips(&mut self, scene: &Scene) -> HashMap<(u32, usize), CasterClip> {
        let mut clips = HashMap::new();
        for id in scene.world.ids_with_mesh() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let slots = scene.world.material(id).map_or(0, |m| m.materials.len());
            for slot in 0..slots {
                if let Some(clip) = self.slot_clip(scene, id, slot) {
                    clips.insert((id, slot), clip);
                }
            }
        }
        clips
    }

    /// How material slot `slot` of entity `id` clips its shadow, or `None` when it
    /// doesn't.
    fn slot_clip(&mut self, scene: &Scene, id: u32, slot: usize) -> Option<CasterClip> {
        let entry = scene.material_entry_at(id, slot)?;
        let asset = entry.1;
        let shader = asset.shader.as_deref();
        let pipeline = self.surface_shaders.pipeline_id(&self.device, shader);
        let cuts = self.surface_shaders.cut(pipeline).is_some();
        let cutout = (asset.render_mode == RenderMode::Cutout)
            .then(|| (asset.alpha_cutoff, asset.base_color_map.is_some()));
        if !cuts && cutout.is_none() {
            return None;
        }
        let clip = Clip {
            pipeline: if cuts { pipeline } else { STANDARD },
            material: self.material_index(Some(entry), pipeline, entity_params(scene, id, slot)),
        };
        Some(CasterClip { clip, cutout })
    }
}
