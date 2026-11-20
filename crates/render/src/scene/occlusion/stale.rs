//! What a scene's baked PVS no longer matches, as its last extract bound it: what
//! `make bench-scenes` names when a scene was measured with a bake older than its
//! statics (`dev::bench::scenes::notes::bakes_unused`).

use rusty_scene::SceneId;

use super::OcclusionBinding;
use crate::{Renderer, StaleOcclusion};

impl OcclusionBinding {
    /// What of the bound PVS no longer matches; `None` with no PVS, or all of it current.
    fn stale(&self) -> Option<StaleOcclusion> {
        self.data.as_ref()?;
        let (changed, occluders) = (self.changed.len(), self.broken.len());
        (changed > 0).then_some(StaleOcclusion { changed, occluders })
    }
}

impl Renderer {
    /// What of scene `id`'s baked PVS no longer matched at its last extract;
    /// `None` when it was never extracted, has no PVS, or all of it matched.
    pub fn stale_occlusion(&self, id: SceneId) -> Option<StaleOcclusion> {
        self.retained.scene(id)?.occlusion.stale()
    }
}
