use super::{SurfacePipelines, SurfaceShaders};
use crate::gpu::pipelines::LazyPipeline;
use rusty_core::fs::written::{written_since, Stale};
use rusty_shadergen::params::sidecar_path;

impl SurfacePipelines {
    fn facing_lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        let cut = self.cut.iter();
        let prepass = cut.clone().flat_map(|c| c.prepass.twins());
        let shadow = cut.map(|c| &c.two_sided_shadow);
        self.forward.twins().chain(prepass).chain(shadow)
    }
}

impl SurfaceShaders {
    /// Returns whether any slot went stale: a retained draw key naming its pipeline must be
    /// re-built, which re-builds the slot.
    pub(crate) fn refresh(&mut self) -> bool {
        let (seen, paths) = written_since(self.seen);
        self.seen = seen;
        if paths.is_empty() {
            return false;
        }
        let written = Stale::new(paths);
        let dirs = &self.dirs;
        let mut any = false;
        for slot in self.slots.iter_mut().filter(|s| !s.stale) {
            slot.stale = dirs.iter().any(|dir| {
                let wgsl = std::path::PathBuf::from(dir).join(format!("{}.wgsl", slot.name));
                written.matches(&wgsl.to_string_lossy())
                    || written.matches(&sidecar_path(&wgsl).to_string_lossy())
            });
            any |= slot.stale;
        }
        any
    }

    pub(crate) fn warm(&mut self) {
        self.warm = true;
        for pipeline in self.lazy_pipelines() {
            pipeline.get();
        }
    }

    pub(crate) fn lazy_pipelines(&self) -> impl Iterator<Item = &LazyPipeline> {
        let built = self.slots.iter().filter_map(|s| s.pipelines.as_ref());
        built.flat_map(SurfacePipelines::facing_lazies)
    }

    pub(super) fn warm_slot(&self, i: usize) {
        let built = self.slots[i].pipelines.as_ref();
        for pipeline in built.into_iter().flat_map(SurfacePipelines::facing_lazies) {
            pipeline.get();
        }
    }
}
