//! The atlas's depth sweeps (#468, #694): re-bake the stale tiles' static casters
//! into the static atlas, then bring the active atlas up to date — each tile that
//! needs it copied from the static one, the dynamic casters drawn over it.

use super::super::casters::{CasterFrame, LightVolume, Sweep};
use super::super::sweeps::depth_pass;
use super::super::ShadowRenderer;
use super::Tile;
use crate::lod::LodSelection;
use crate::scene::OcclusionBinding;

/// How the frame's tiles got their static casters: from the cache, or re-baked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::passes::shadows) struct AtlasBake {
    pub cached: u32,
    pub rebaked: u32,
}

impl ShadowRenderer {
    /// Record the atlas's sweeps for this frame's tiles: re-bake the stale tiles'
    /// static casters (always at LOD0, as the cascades' bake), then, per tile that
    /// needs it, copy its statics into the active atlas and draw the dynamic casters
    /// `lod` shows over them. A tile no dynamic caster reaches, whose active texels
    /// already hold its current bake, is left as it is (#1711): the forward shader
    /// samples the same depths either way.
    pub(in crate::passes::shadows) fn render_atlas(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &CasterFrame,
        lod: &LodSelection,
    ) -> AtlasBake {
        let tiles = std::mem::take(&mut self.atlas.frame_tiles);
        let bake = self.bake_static_tiles(encoder, frame, &tiles);
        if !tiles.is_empty() {
            let pvs = &frame.objects.occlusion;
            let volumes: Vec<LightVolume> = tiles.iter().map(|t| volume(pvs, t)).collect();
            let batches = self.prepare_casters(frame, lod, Sweep::AtlasDynamic, &volumes);
            let scene = frame.statics;
            let clean = &self.atlas.clean_tiles;
            let fresh = self.atlas.fresh;
            let work: Vec<usize> = (0..tiles.len())
                .filter(|&i| fresh || !batches[i].is_empty() || !clean.holds(scene, &tiles[i]))
                .collect();
            if !work.is_empty() {
                let view = &self.atlas.view;
                let mut pass = depth_pass(encoder, frame, "Shadow Atlas Pass", view, fresh);
                for &i in &work {
                    focus(&mut pass, &tiles[i]);
                    self.atlas.blit.copy(&mut pass);
                    self.draw_casters(&mut pass, frame, &batches[i], Sweep::AtlasDynamic, i);
                }
            }
            self.atlas.fresh &= work.is_empty();
            for &i in &work {
                // Drawn over: clean again only when nothing dynamic was drawn.
                self.atlas.clean_tiles.forget(&tiles[i]);
                if batches[i].is_empty() {
                    self.atlas.clean_tiles.commit(scene, [tiles[i]]);
                }
            }
        }
        self.atlas.frame_tiles = tiles;
        bake
    }

    /// Re-bake the static casters of every tile the static atlas does not hold for
    /// this scene: clear the tile, then draw its static casters into it.
    fn bake_static_tiles(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &CasterFrame,
        tiles: &[Tile],
    ) -> AtlasBake {
        let scene = frame.statics;
        let stale = self.atlas.static_tiles.stale(scene, tiles);
        let bake = AtlasBake {
            cached: (tiles.len() - stale.len()) as u32,
            rebaked: stale.len() as u32,
        };
        if stale.is_empty() {
            return bake;
        }
        // The bake outlives the frame, so it cannot follow the camera's LOD choice.
        let finest = LodSelection::finest(frame.scene);
        // A shadowmasked tile keeps its statics out (#1695): it is cleared only,
        // and no static caster is culled for it.
        let drawn: Vec<usize> = stale
            .iter()
            .copied()
            .filter(|&i| tiles[i].statics)
            .collect();
        let pvs = &frame.objects.occlusion;
        let volumes: Vec<LightVolume> = drawn.iter().map(|&i| volume(pvs, &tiles[i])).collect();
        let batches = match volumes.is_empty() {
            true => Vec::new(),
            false => self.prepare_casters(frame, &finest, Sweep::AtlasStatic, &volumes),
        };
        {
            let view = &self.atlas.static_view;
            let mut pass = depth_pass(encoder, frame, "Shadow Atlas Static Pass", view, false);
            for &i in &stale {
                focus(&mut pass, &tiles[i]);
                self.atlas.blit.clear(&mut pass);
            }
            for (&i, batches) in drawn.iter().zip(&batches) {
                focus(&mut pass, &tiles[i]);
                self.draw_casters(&mut pass, frame, batches, Sweep::AtlasStatic, i);
            }
        }
        let rebaked = stale.iter().map(|&i| tiles[i]);
        self.atlas.static_tiles.commit(scene, rebaked);
        // The active atlas's copy of a re-baked tile is the old bake's.
        stale
            .iter()
            .for_each(|&i| self.atlas.clean_tiles.forget(&tiles[i]));
        bake
    }
}

/// `tile`'s light volume: culled by its light's PVS row (#1714) only while its
/// static casters are in it. A shadowmasked tile's walls are the mask's, not
/// drawn, so its light sees past them and it culls nothing by occlusion.
pub(super) fn volume(pvs: &OcclusionBinding, tile: &Tile) -> LightVolume {
    LightVolume {
        view_proj: tile.view_proj,
        row: tile.statics.then(|| pvs.shadow_row(tile.light)).flatten(),
    }
}

/// Point the pass's viewport and scissor at `tile`.
fn focus(pass: &mut wgpu::RenderPass, tile: &Tile) {
    let [x, y] = tile.origin;
    let size = tile.size as f32;
    pass.set_viewport(x as f32, y as f32, size, size, 0.0, 1.0);
    pass.set_scissor_rect(x, y, tile.size, tile.size);
}
