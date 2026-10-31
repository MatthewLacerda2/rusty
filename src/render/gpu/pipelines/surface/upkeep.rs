//! Keeping the surface-shader cache current: hot reload marks a slot stale when its
//! file changes (#396, #1327), and a windowed renderer builds a slot's facing
//! pipelines with it (#1615), so a mirrored or two-sided draw never compiles one
//! mid-frame.

use super::{SurfacePipelines, SurfaceShaders};
use crate::core::fs::written::{written_since, Stale};
use crate::core::paths::native;
use crate::render::gpu::pipelines::LazyPipeline;
use crate::shadergen::params::sidecar_path;

impl SurfacePipelines {
    /// Its lazy pipelines by facing (#1615): the opaque pass's and the cut prepass's
    /// mirrored and two-sided ones, and the cut's two-sided shadow (#1571).
    fn facing_lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        let cut = self.cut.iter();
        let prepass = cut.clone().flat_map(|c| c.prepass.twins());
        let shadow = cut.map(|c| &c.two_sided_shadow);
        self.forward.twins().chain(prepass).chain(shadow)
    }
}

impl SurfaceShaders {
    /// Mark every slot stale whose module or params file, in any of the dirs it
    /// resolves against, was written since the last look (re-baked, appeared, edited,
    /// vanished).
    pub(crate) fn refresh(&mut self) {
        let (seen, paths) = written_since(self.seen);
        self.seen = seen;
        if paths.is_empty() {
            return;
        }
        let written = Stale::new(paths);
        let dirs = &self.dirs;
        for slot in self.slots.iter_mut().filter(|s| !s.stale) {
            slot.stale = dirs.iter().any(|dir| {
                let wgsl = native(dir).join(format!("{}.wgsl", slot.name));
                written.matches(&wgsl.to_string_lossy())
                    || written.matches(&sidecar_path(&wgsl).to_string_lossy())
            });
        }
    }

    /// From now on build each slot's facing pipelines with it, and build those of
    /// the slots already built: what `Renderer::warm_pipelines` asks of a windowed
    /// renderer, whose slots are built after it starts.
    pub(crate) fn warm(&mut self) {
        self.warm = true;
        for pipeline in self.lazy_pipelines() {
            pipeline.get();
        }
    }

    /// The facing pipelines of every slot built so far (#1615), listed with the
    /// renderer's own lazy pipelines (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> impl Iterator<Item = &LazyPipeline> {
        let built = self.slots.iter().filter_map(|s| s.pipelines.as_ref());
        built.flat_map(SurfacePipelines::facing_lazies)
    }

    /// Build slot `i`'s facing pipelines now.
    pub(super) fn warm_slot(&self, i: usize) {
        let built = self.slots[i].pipelines.as_ref();
        for pipeline in built.into_iter().flat_map(SurfacePipelines::facing_lazies) {
            pipeline.get();
        }
    }
}
