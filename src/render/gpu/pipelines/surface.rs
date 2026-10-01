//! src/render/gpu/pipelines/surface.rs — the pipelines a material's authored surface
//! shader draws with (#396): Unity's Material → Shader.
//!
//! A material names a surface shader (`MaterialAsset::shader`, e.g. `"enemy_toon"`);
//! the module is `<name>.wgsl` in the authored workspace (`project/assets/shaders`,
//! where `Shader.Bake` writes), else the engine set (`assets/shaders`). A surface
//! variant keeps the forward contract by construction (#272: same `VertexInput`, bind
//! groups and entry points), so only the shader module differs: its pipelines reuse
//! the forward pipeline layout, and a variant is two pipelines — opaque and
//! transparent. A variant's runtime params (#399) ride the material's group-2 param
//! uniform; the slot keeps the variant's [`ParamLayout`] (its `<name>.params.json`)
//! so the material cache can pack a material's values into it.
//!
//! **Pipeline ids.** Every draw carries a `usize` pipeline id: [`STANDARD`] (0) is the
//! renderer's own forward/transparent pair; `n > 0` is cache slot `n - 1`. The id is
//! the first field of the batch key, so opaque draws sort into one run per shader.
//!
//! **Lazy, fail-safe.** A slot is built the first frame a material names it. A module
//! that is missing, fails to compose, lacks `vs_main`/`fs_main` (a postfx module), or
//! fails wgpu validation is logged **once** and draws with the standard pipeline —
//! a bad shader never crashes a frame.
//!
//! **Hot reload.** [`SurfaceShaders::refresh`] re-stats each slot's file once per
//! render; a re-bake (new mtime or length) or a module appearing in the workspace marks
//! the slot stale, and the next draw that names it rebuilds — the agent's bake → look
//! → iterate loop works in a live session.
//!
//! **Depth prepass.** Variants ride the standard SSAO prepass (#436): their `vs_main`
//! is the forward one verbatim and blocks only restyle colour, so depth is identical.
//! The shadow pass is likewise unaffected.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::render::gpu::shaders::ShaderRegistry;
use crate::shadergen::params::ParamLayout;
use crate::shadergen::{DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

/// The pipeline id of the renderer's standard forward shader.
pub(crate) const STANDARD: usize = 0;

/// One authored surface shader's pipelines.
pub(crate) struct SurfacePipelines {
    /// Opaque/cutout solids (the forward spec).
    pub forward: wgpu::RenderPipeline,
    /// Alpha-blended solids (the transparent spec).
    pub transparent: wgpu::RenderPipeline,
}

/// Which solids pass a batch is drawn in, carrying that pass's standard pipeline, so
/// [`SurfaceShaders::pick`] can swap in a material's surface variant (#396).
#[derive(Clone, Copy)]
pub(crate) enum SolidPass<'a> {
    /// The SSAO depth prepass: one pipeline for every batch (variants share depth).
    Prepass,
    /// Opaque solids; the standard pipeline is the view's (the preview may swap it).
    Opaque(&'a wgpu::RenderPipeline),
    /// Alpha-blended solids.
    Transparent(&'a wgpu::RenderPipeline),
}

/// What a slot's file looked like when it was last built: its path, mtime and length.
type Stamp = Option<(PathBuf, Option<SystemTime>, u64)>;

struct Slot {
    name: String,
    stamp: Stamp,
    /// `None` when the module failed (it was logged; draws fall back to [`STANDARD`]).
    pipelines: Option<SurfacePipelines>,
    /// The module's runtime params (#399); empty when it has none.
    params: ParamLayout,
    /// The file changed since the build; rebuild on the next lookup.
    stale: bool,
}

/// The pipeline cache, keyed by shader name.
pub(crate) struct SurfaceShaders {
    layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    /// Where a name resolves, searched in order (authored workspace first).
    dirs: Vec<String>,
    index: HashMap<String, usize>,
    slots: Vec<Slot>,
    /// Modules built so far — a runtime param change must never add one (#399).
    builds: usize,
}

impl SurfaceShaders {
    /// An empty cache whose variants render into `format` through `layout` (the
    /// forward pipeline layout).
    pub(crate) fn new(layout: wgpu::PipelineLayout, format: wgpu::TextureFormat) -> Self {
        Self {
            layout,
            format,
            dirs: vec![DEFAULT_OUT_DIR.into(), ENGINE_SHADER_DIR.into()],
            index: HashMap::new(),
            slots: Vec::new(),
            builds: 0,
        }
    }

    /// Point name resolution at `dirs` instead of the default workspace + engine set.
    #[cfg(test)]
    pub(crate) fn set_dirs(&mut self, dirs: Vec<String>) {
        self.dirs = dirs;
    }

    /// The pipeline id for a material's `shader`: [`STANDARD`] for `None` or a module
    /// that could not be built, else its cache slot's id (building it on first use).
    pub(crate) fn pipeline_id(&mut self, device: &wgpu::Device, shader: Option<&str>) -> usize {
        let Some(name) = shader else {
            return STANDARD;
        };
        let slot = match self.index.get(name) {
            Some(&i) => i,
            None => {
                self.slots.push(Slot {
                    name: name.to_owned(),
                    stamp: None,
                    pipelines: None,
                    params: ParamLayout::default(),
                    stale: true,
                });
                self.index.insert(name.to_owned(), self.slots.len() - 1);
                self.slots.len() - 1
            }
        };
        if self.slots[slot].stale {
            self.rebuild(device, slot);
        }
        match self.slots[slot].pipelines {
            Some(_) => slot + 1,
            None => STANDARD,
        }
    }

    /// The runtime params of pipeline `id`'s module (#399): `None` for the standard
    /// shader or a module with none.
    pub(crate) fn params(&self, id: usize) -> Option<&ParamLayout> {
        let slot = self.slots.get(id.checked_sub(1)?)?;
        (!slot.params.params.is_empty()).then_some(&slot.params)
    }

    /// Modules built so far (each build or rebuild of a slot counts one).
    #[cfg(test)]
    pub(crate) fn builds(&self) -> usize {
        self.builds
    }

    /// The pipelines behind a non-standard id from [`Self::pipeline_id`] this frame.
    fn get(&self, id: usize) -> Option<&SurfacePipelines> {
        self.slots.get(id.checked_sub(1)?)?.pipelines.as_ref()
    }

    /// The pipeline pipeline `id` draws with in `pass`: its variant's, else the pass's
    /// standard one. `None` keeps the bound pipeline (the prepass).
    pub(crate) fn pick<'a>(
        &'a self,
        pass: SolidPass<'a>,
        id: usize,
    ) -> Option<&'a wgpu::RenderPipeline> {
        let variant = self.get(id);
        match pass {
            SolidPass::Prepass => None,
            SolidPass::Opaque(standard) => Some(variant.map_or(standard, |v| &v.forward)),
            SolidPass::Transparent(standard) => Some(variant.map_or(standard, |v| &v.transparent)),
        }
    }

    /// Mark every slot whose file changed (re-baked, appeared, vanished) stale.
    pub(crate) fn refresh(&mut self) {
        for i in 0..self.slots.len() {
            if !self.slots[i].stale && self.stamp(&self.slots[i].name) != self.slots[i].stamp {
                self.slots[i].stale = true;
            }
        }
    }

    /// Rebuild slot `i` from its current file, logging once per failed version.
    fn rebuild(&mut self, device: &wgpu::Device, i: usize) {
        let stamp = self.stamp(&self.slots[i].name);
        self.builds += 1;
        let built = match &stamp {
            Some((path, ..)) => self
                .build(device, path)
                .and_then(|pipelines| Ok((pipelines, ParamLayout::read_beside(path)?))),
            None => Err(format!(
                "no `{}.wgsl` in {}",
                self.slots[i].name,
                self.dirs.join(" or ")
            )),
        };
        let slot = &mut self.slots[i];
        let built = built
            .map_err(|e| {
                log::warn!(
                    "surface shader {:?}: {e}; using the standard shader",
                    slot.name
                );
            })
            .ok();
        (slot.pipelines, slot.params) = match built {
            Some((pipelines, params)) => (Some(pipelines), params),
            None => (None, ParamLayout::default()),
        };
        slot.stamp = stamp;
        slot.stale = false;
    }

    /// Where `name` resolves now, with its mtime and length; `None` if nowhere (or the
    /// name is not a bare module name, so it can never escape the shader dirs).
    fn stamp(&self, name: &str) -> Stamp {
        if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
            return None;
        }
        self.dirs.iter().find_map(|dir| {
            let path = PathBuf::from(format!("{dir}/{name}.wgsl"));
            let meta = std::fs::metadata(&path).ok()?;
            Some((path, meta.modified().ok(), meta.len()))
        })
    }

    /// Compose `path` against `common`, check it is a surface module, and build its
    /// pipelines under a validation error scope, so wgpu refuses instead of panicking.
    fn build(&self, device: &wgpu::Device, path: &PathBuf) -> Result<SurfacePipelines, String> {
        let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut composer = ShaderRegistry::composer_with_common(ENGINE_SHADER_DIR)?;
        let module = ShaderRegistry::validate_source(&mut composer, &source)?;
        for (entry, stage) in [
            ("vs_main", wgpu::naga::ShaderStage::Vertex),
            ("fs_main", wgpu::naga::ShaderStage::Fragment),
        ] {
            if !module
                .entry_points
                .iter()
                .any(|e| e.name == entry && e.stage == stage)
            {
                return Err(format!("not a surface shader (no {entry})"));
            }
        }
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Surface Variant Shader"),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module)),
        });
        let pipelines = super::surface_pipelines(device, &shader, self.format, &self.layout);
        match pollster::block_on(device.pop_error_scope()) {
            Some(e) => Err(e.to_string()),
            None => Ok(pipelines),
        }
    }
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod surface_tests;

#[cfg(test)]
#[path = "params_tests.rs"]
mod params_tests;
