//! src/render/gpu/pipelines/surface.rs — the pipelines a material's authored surface
//! shader draws with (#396): Unity's Material → Shader.
//!
//! A material names a surface shader (`MaterialAsset::shader`, e.g. `"enemy_toon"`);
//! the module is `<name>.wgsl` in the authored workspace (`assets/shaders`,
//! where `Shader.Bake` writes), else the engine set (`engine/shaders`). A surface
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
//! **Hot reload.** [`SurfaceShaders::refresh`] drains the write log
//! (`core::fs::written`) once per render: a slot whose module or params file was
//! written — re-baked (`Shader.Bake` notes it), appeared, edited or removed outside the
//! engine (the editor's file watch notes those, #1327) — goes stale, and the next draw
//! that names it rebuilds. The agent's bake → look → iterate loop works in a live
//! session, and costs nothing on a frame nothing was written.
//!
//! **Depth passes.** A variant's `vs_main` is the forward one verbatim, so one whose
//! blocks only restyle colour rides the standard SSAO prepass (#436) and shadow
//! pipelines. One whose blocks `discard` (`dissolve`, #400) also builds
//! [`CutPipelines`] from its own `fs_prepass_cut` / `fs_shadow_cut` (#648): the same
//! cut, reading the same runtime params and mask, so a dissolved fragment casts no
//! shadow and fills no SSAO depth. Only those variants pay for a depth fragment.

use std::collections::HashMap;
use std::path::Path;

use crate::core::fs::written::{written_since, Stale};
use crate::core::paths::native;
use crate::shadergen::assemble::{CUT_PREPASS, CUT_SHADOW};
use crate::shadergen::params::{resolve_module, sidecar_path, ParamLayout};
use crate::shadergen::{compose, engine_shader_dir, DEFAULT_OUT_DIR};

/// The pipeline id of the renderer's standard forward shader.
pub(crate) const STANDARD: usize = 0;

/// One authored surface shader's pipelines.
pub(crate) struct SurfacePipelines {
    /// Opaque/cutout solids (the forward spec).
    pub forward: wgpu::RenderPipeline,
    /// Alpha-blended solids (the transparent spec).
    pub transparent: wgpu::RenderPipeline,
    /// The depth passes of a variant whose blocks cut fragments (#648); `None` draws
    /// its depth with the standard pipelines.
    pub cut: Option<CutPipelines>,
}

/// A cutting variant's depth-only pipelines (#648); the shadow ones go through the
/// shadow pass's clip layout, `mirrored_shadow` with the winding flipped (#1134).
pub(crate) struct CutPipelines {
    /// The SSAO prepass, through the forward layout.
    pub prepass: wgpu::RenderPipeline,
    pub shadow: wgpu::RenderPipeline,
    pub mirrored_shadow: wgpu::RenderPipeline,
}

/// Which solids pass a batch is drawn in, carrying that pass's standard pipeline, so
/// [`SurfaceShaders::pick`] can swap in a material's surface variant (#396).
#[derive(Clone, Copy)]
pub(crate) enum SolidPass<'a> {
    /// The SSAO depth prepass; the standard pipeline unless a variant cuts (#648).
    Prepass(&'a wgpu::RenderPipeline),
    /// Opaque solids; the standard pipeline is the view's (the preview may swap it).
    Opaque(&'a wgpu::RenderPipeline),
    /// Alpha-blended solids; the standard pipeline is built only if a batch uses it.
    Transparent(&'a super::LazyPipeline),
}

struct Slot {
    name: String,
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
    /// The shadow pass's clip layout (light, casters, material), for cut variants.
    shadow_layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    /// Where a name resolves, searched in order (authored workspace first).
    dirs: Vec<String>,
    index: HashMap<String, usize>,
    slots: Vec<Slot>,
    /// Modules built so far — a runtime param change must never add one (#399).
    builds: usize,
    /// The newest write-log generation already looked at.
    seen: u64,
}

impl SurfaceShaders {
    /// An empty cache: variants render into `format` through `layout` (the forward
    /// layout), and cast clipped shadows through `shadow_layout`.
    pub(crate) fn new(
        layout: wgpu::PipelineLayout,
        shadow_layout: wgpu::PipelineLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            layout,
            shadow_layout,
            format,
            dirs: vec![DEFAULT_OUT_DIR.into(), engine_shader_dir().into()],
            index: HashMap::new(),
            slots: Vec::new(),
            builds: 0,
            seen: written_since(u64::MAX).0,
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

    /// What pipeline `id` draws with in `pass`: its variant's, else the standard one.
    pub(crate) fn pick<'a>(&'a self, pass: SolidPass<'a>, id: usize) -> &'a wgpu::RenderPipeline {
        let variant = self.get(id);
        match pass {
            SolidPass::Prepass(standard) => self.cut(id).map_or(standard, |c| &c.prepass),
            SolidPass::Opaque(standard) => variant.map_or(standard, |v| &v.forward),
            SolidPass::Transparent(lazy) => variant.map_or_else(|| lazy.get(), |v| &v.transparent),
        }
    }

    /// The depth pipelines of pipeline `id`, if its variant cuts fragments (#648).
    pub(crate) fn cut(&self, id: usize) -> Option<&CutPipelines> {
        self.get(id)?.cut.as_ref()
    }

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

    /// Rebuild slot `i` from its current file, logging once per failed version.
    fn rebuild(&mut self, device: &wgpu::Device, i: usize) {
        let dirs: Vec<&str> = self.dirs.iter().map(String::as_str).collect();
        let module = resolve_module(&dirs, &self.slots[i].name);
        self.builds += 1;
        let built = match &module {
            Some(path) => self
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
                    "[Render] surface shader {:?}: {e}; using the standard shader",
                    slot.name
                );
            })
            .ok();
        (slot.pipelines, slot.params) = match built {
            Some((pipelines, params)) => (Some(pipelines), params),
            None => (None, ParamLayout::default()),
        };
        slot.stale = false;
    }

    /// Compose `path` against `common`, check it is a surface module, and build its
    /// pipelines under a validation error scope, so wgpu refuses instead of panicking.
    fn build(&self, device: &wgpu::Device, path: &Path) -> Result<SurfacePipelines, String> {
        let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut composer = compose::composer_with_common(engine_shader_dir())?;
        let module = compose::compose(&mut composer, &source, &path.to_string_lossy())?;
        let has = |entry: &str, stage| {
            module
                .entry_points
                .iter()
                .any(|e| e.name == entry && e.stage == stage)
        };
        for (entry, stage) in [
            ("vs_main", wgpu::naga::ShaderStage::Vertex),
            ("fs_main", wgpu::naga::ShaderStage::Fragment),
        ] {
            if !has(entry, stage) {
                return Err(format!("not a surface shader (no {entry})"));
            }
        }
        let fragment = wgpu::naga::ShaderStage::Fragment;
        let cuts = has(CUT_PREPASS, fragment) && has(CUT_SHADOW, fragment);
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Surface Variant Shader"),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module)),
        });
        let layouts = [&self.layout, &self.shadow_layout];
        let pipelines = super::surface_pipelines(device, &shader, self.format, layouts, cuts);
        match pollster::block_on(scope.pop()) {
            Some(e) => Err(e.to_string()),
            None => Ok(pipelines),
        }
    }
}

#[cfg(test)]
#[path = "tests/surface_tests.rs"]
mod surface_tests;

#[cfg(test)]
#[path = "tests/params_tests.rs"]
mod params_tests;

#[cfg(test)]
#[path = "tests/mask_tests.rs"]
pub(crate) mod mask_tests;

#[cfg(test)]
#[path = "tests/gameplay_blocks_tests.rs"]
mod gameplay_blocks_tests;

#[cfg(test)]
#[path = "tests/cut_depth_tests.rs"]
pub(crate) mod cut_depth_tests;
