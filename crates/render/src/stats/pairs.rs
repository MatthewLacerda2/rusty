//! [`RenderCounters`] under their `FrameStats` metric names.

use super::RenderCounters;

/// [`RenderCounters::ui_relaid`]'s metric names, in its order.
const UI_RELAID: [&str; 7] = [
    "ui_relaid_rebuild",
    "ui_relaid_screen",
    "ui_relaid_textures",
    "ui_relaid_structure",
    "ui_relaid_content",
    "ui_relaid_transform",
    "ui_relaid_marker",
];

impl RenderCounters {
    /// Every counter under its `FrameStats` metric name.
    pub fn pairs(&self) -> Vec<(&'static str, u64)> {
        let mut pairs = vec![
            ("draw_calls", self.draw_calls.into()),
            ("triangles", self.triangles),
            ("visible_entities", self.visible_entities.into()),
            ("cull_tested", self.cull_tested.into()),
            ("culled_entities", self.culled_entities.into()),
            ("size_culled_entities", self.size_culled_entities.into()),
        ];
        pairs.extend(self.occlusion_pairs());
        pairs.extend([
            ("lod_hidden_entities", self.lod_hidden_entities.into()),
            ("lights", self.lights.into()),
            ("lights_dropped", self.lights_dropped.into()),
            ("cluster_lights_dropped", self.cluster_lights_dropped.into()),
            ("lights_visible", self.lights_visible.into()),
            ("lights_culled", self.lights_culled.into()),
            ("light_cluster_refs", self.light_cluster_refs),
            ("light_bin_us", self.light_bin_us),
            ("decals_visible", self.decals_visible.into()),
            ("decal_cluster_refs", self.decal_cluster_refs),
            ("decal_maps_dropped", self.decal_maps_dropped.into()),
            ("ssao_samples", self.ssao_samples),
            ("ssil_samples", self.ssil_samples),
            ("volumetric_froxels", self.volumetric_froxels),
            ("particles_drawn", self.particles_drawn.into()),
        ]);
        pairs.extend(self.forward_pairs());
        pairs.extend([
            ("history_passes", self.history_passes.into()),
            ("ssr_surface_passes", self.ssr_surface_passes.into()),
            ("grade_lut_bakes", self.grade_lut_bakes.into()),
            ("sky_reflection_bakes", self.sky_reflection_bakes.into()),
            ("opaque_copies", self.opaque_copies.into()),
            ("texture_bytes", self.texture_bytes),
            ("lightmap_bytes", self.lightmap_bytes),
            ("reflection_bytes", self.reflection_bytes),
            ("mesh_count", self.mesh_count.into()),
            ("mesh_bytes", self.mesh_bytes),
            ("cluster_pool_bytes", self.cluster_pool_bytes),
            ("render_extracted", self.render_extracted.into()),
            ("render_posed", self.render_posed.into()),
            ("render_extract_us", self.render_extract_us),
            ("render_upload_bytes", self.render_upload_bytes),
        ]);
        pairs.extend(self.shadow_pairs());
        pairs.extend(self.ui_pairs());
        pairs.extend(UI_RELAID.into_iter().zip(self.ui_relaid.map(u64::from)));
        pairs.extend(self.uploads.pairs());
        pairs.extend(self.ops.pairs());
        pairs
    }

    /// The camera views' occlusion counters (the PVS's, Hi-Z's and the cluster
    /// cull's), under their metric names.
    fn occlusion_pairs(&self) -> [(&'static str, u64); 8] {
        [
            ("occlusion_culled", self.occlusion_culled.into()),
            ("hiz_tested", self.hiz_tested.into()),
            ("hiz_culled", self.hiz_culled.into()),
            ("hiz_late", self.hiz_late.into()),
            ("hiz_depth_triangles", self.hiz_depth_triangles),
            ("cluster_tested", self.cluster_tested.into()),
            ("cluster_culled", self.cluster_culled.into()),
            ("cluster_draws", self.cluster_draws.into()),
        ]
    }

    /// The render-texture cameras, the opaque scene passes' kinds and their draws
    /// by kind (#2334), under their metric names.
    fn forward_pairs(&self) -> [(&'static str, u64); 6] {
        [
            ("render_texture_draws", self.render_texture_draws.into()),
            ("velocity_passes", self.velocity_passes.into()),
            ("primed_passes", self.primed_passes.into()),
            ("forward_lit_primed", self.forward_lit_primed.into()),
            ("forward_lit_unprimed", self.forward_lit_unprimed.into()),
            ("forward_unlit", self.forward_unlit.into()),
        ]
    }

    /// The shadow views' counters, under their metric names.
    fn shadow_pairs(&self) -> [(&'static str, u64); 12] {
        [
            ("shadow_cull_tested", self.shadow_cull_tested.into()),
            ("shadow_frustum_culled", self.shadow_frustum_culled.into()),
            ("shadow_size_culled", self.shadow_size_culled.into()),
            (
                "shadow_occlusion_culled",
                self.shadow_occlusion_culled.into(),
            ),
            ("shadow_gather_us", self.shadow_gather_us),
            ("shadow_draws", self.shadow_draws.into()),
            ("shadowed_lights", self.shadowed_lights.into()),
            ("shadow_lights_dropped", self.shadow_lights_dropped.into()),
            ("shadow_atlas_tiles", self.shadow_atlas_tiles.into()),
            ("shadow_atlas_texels", self.shadow_atlas_texels),
            ("shadow_atlas_cached", self.shadow_atlas_cached.into()),
            ("shadow_atlas_rebaked", self.shadow_atlas_rebaked.into()),
        ]
    }

    /// The UI pass's counters, under their metric names.
    fn ui_pairs(&self) -> [(&'static str, u64); 5] {
        [
            ("ui_draws", self.ui_draws.into()),
            ("ui_meshed", self.ui_meshed.into()),
            ("ui_laid_out", self.ui_laid_out.into()),
            ("ui_mask_passes", self.ui_mask_passes.into()),
            ("ui_blur_passes", self.ui_blur_passes.into()),
        ]
    }
}
