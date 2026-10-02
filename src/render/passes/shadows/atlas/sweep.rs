//! The atlas's depth sweeps (#468, #694): re-bake the stale tiles' static casters
//! into the static atlas, then build the active atlas — each tile copied from the
//! static one, the dynamic casters drawn over it.

use glam::Mat4;

use super::super::casters::{CasterFrame, Sweep};
use super::super::sweeps::depth_pass;
use super::super::ShadowRenderer;
use super::Tile;
use crate::render::lod::LodSelection;

/// How the frame's tiles got their static casters: from the cache, or re-baked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::render::passes::shadows) struct AtlasBake {
    pub cached: u32,
    pub rebaked: u32,
}

impl ShadowRenderer {
    /// Record the atlas's sweeps for this frame's tiles: re-bake the stale tiles'
    /// static casters (always at LOD0, as the cascades' bake), then clear the active
    /// atlas and, per tile, copy its statics in and draw the dynamic casters `lod`
    /// shows over them. Nothing when no light is shadowed.
    pub(in crate::render::passes::shadows) fn render_atlas(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &CasterFrame,
        lod: &LodSelection,
    ) -> AtlasBake {
        let tiles = std::mem::take(&mut self.atlas.frame_tiles);
        let bake = self.bake_static_tiles(encoder, frame, &tiles);
        if !tiles.is_empty() {
            let volumes: Vec<Mat4> = tiles.iter().map(|t| t.view_proj).collect();
            let batches = self.prepare_casters(frame, lod, Sweep::AtlasDynamic, &volumes);
            let view = &self.atlas.view;
            let mut pass = depth_pass(encoder, "Shadow Atlas Pass", view, true);
            for (i, (tile, batches)) in tiles.iter().zip(&batches).enumerate() {
                focus(&mut pass, tile);
                self.atlas.blit.copy(&mut pass);
                self.draw_casters(&mut pass, frame, batches, Sweep::AtlasDynamic, i);
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
        let scene = frame.scene.id();
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
        let volumes: Vec<Mat4> = stale.iter().map(|&i| tiles[i].view_proj).collect();
        let batches = self.prepare_casters(frame, &finest, Sweep::AtlasStatic, &volumes);
        {
            let view = &self.atlas.static_view;
            let mut pass = depth_pass(encoder, "Shadow Atlas Static Pass", view, false);
            for (&i, batches) in stale.iter().zip(&batches) {
                focus(&mut pass, &tiles[i]);
                self.atlas.blit.clear(&mut pass);
                self.draw_casters(&mut pass, frame, batches, Sweep::AtlasStatic, i);
            }
        }
        let rebaked = stale.iter().map(|&i| tiles[i]);
        self.atlas.static_tiles.commit(scene, rebaked);
        bake
    }
}

/// Point the pass's viewport and scissor at `tile`.
fn focus(pass: &mut wgpu::RenderPass, tile: &Tile) {
    let [x, y] = tile.origin;
    let size = tile.size as f32;
    pass.set_viewport(x as f32, y as f32, size, size, 0.0, 1.0);
    pass.set_scissor_rect(x, y, tile.size, tile.size);
}
