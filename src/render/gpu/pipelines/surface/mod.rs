//! The pipelines a material's authored surface shader draws with (#396): Unity's
//! Material → Shader.
//!
//! A material names a surface shader (`MaterialAsset::shader`, e.g. `"enemy_toon"`);
//! the module is `<name>.wgsl` in the authored workspace (`assets/shaders`, where
//! `Shader.Bake` writes), else the engine set (`engine/shaders`). A surface
//! variant keeps the forward contract by construction (#272: same `VertexInput`, bind
//! groups and entry points), so only the module differs: its pipelines (opaque, motion
//! (#1435), one transparent per blend and the depth postpass (#1523)) reuse the
//! forward layout. Its runtime params (#399) ride the material's group-2 uniform, packed
//! by the slot's [`ParamLayout`] (`<name>.params.json`).
//!
//! **Pipeline ids.** Every draw carries a `usize` pipeline id: [`STANDARD`] (0) is the
//! renderer's own forward/transparent pair; `n > 0` is cache slot `n - 1`. The id is
//! the first field of the batch key, so opaque draws sort into one run per shader.
//!
//! **Lazy, fail-safe.** A slot is built the first frame a material names it. A module
//! that is missing, fails to compose, lacks `vs_main`/`fs_main` (a postfx module) or
//! the motion, SSR or depth-postpass entries (#1435, #1526, #1523), or fails
//! validation is logged **once** and draws with the standard pipeline.
//!
//! **Hot reload.** [`SurfaceShaders::refresh`] drains the write log
//! (`core::fs::written`) once per render: a slot whose module or params file was
//! written — re-baked (`Shader.Bake` notes it), appeared, edited or removed outside the
//! engine (the editor's file watch notes those, #1327) — goes stale, and the next draw
//! that names it rebuilds: bake → look → iterate in a live session, free otherwise.
//!
//! **Depth passes.** A variant whose blocks only restyle colour keeps the forward
//! vertex stage as it is, so it rides the standard SSAO prepass (#436) and shadow
//! pipelines. One whose blocks `discard` (`dissolve`, #400), move the UVs
//! (`uv_scroll`, #1499) or move the vertices (`vertex_waves`, #1524) also builds
//! [`CutPipelines`] from its own `fs_prepass_cut` / `fs_shadow_cut` (#648), over its
//! own `vs_main` / `vs_shadow`: the same cut, UVs and vertices, reading the same
//! runtime params and mask, so a dissolved fragment casts no shadow and fills no SSAO
//! depth, a scrolled Cutout texel clips where it shows, and a swell's shadow and depth
//! follow it. Only those variants pay for depth pipelines of their own.

mod pass;
mod upkeep;

use std::{collections::HashMap, path::Path};
use wgpu::naga::ShaderStage;

use crate::core::fs::written::written_since;
use crate::shadergen::assemble::{CUT_PREPASS, CUT_SHADOW};
use crate::shadergen::params::{resolve_module, ParamLayout};
use crate::shadergen::{compose, engine_shader_dir, DEFAULT_OUT_DIR};

/// The pipeline id of the renderer's standard forward shader.
pub(crate) const STANDARD: usize = 0;

pub(crate) use pass::SolidPass;

/// One authored surface shader's pipelines.
pub(crate) struct SurfacePipelines {
    /// Opaque/cutout solids per facing (#1615), and into a velocity target (#1435).
    pub forward: super::FacedPipeline,
    pub velocity: super::VelocityPipelines,
    /// The same two over a primed depth (#1499), built the first time a view primes.
    pub primed: super::PrimedPipelines,
    /// Blended solids, one per blend state (#1523), built on first draw.
    pub transparent: super::TransparentPipelines,
    /// Its translucents' depth postpass (#1523), built on first draw.
    pub depth_postpass: super::DepthPostpassPipelines,
    /// Its depth passes when its blocks cut or move (#648); `None` uses the standard ones.
    pub cut: Option<CutPipelines>,
}

/// The depth-only pipelines of a variant that cuts, moves its UVs or its vertices
/// (#648, #1499, #1524); the shadow ones through the shadow pass's clip layout, one
/// per [`Facing`](super::winding::Facing) (#1134; the two-sided one lazy, #1571).
pub(crate) struct CutPipelines {
    /// The SSAO prepass, through the forward layout, one per facing (#1615).
    pub prepass: super::FacedPipeline,
    pub shadow: wgpu::RenderPipeline,
    pub mirrored_shadow: wgpu::RenderPipeline,
    pub two_sided_shadow: super::LazyPipeline,
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
    /// Build a slot's facing pipelines with it (a windowed renderer, #1615).
    warm: bool,
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
            warm: false,
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

    /// The depth pipelines of pipeline `id`, if its variant cuts fragments or moves
    /// its UVs (#648, #1499).
    pub(crate) fn cut(&self, id: usize) -> Option<&CutPipelines> {
        self.get(id)?.cut.as_ref()
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
        if self.warm {
            self.warm_slot(i);
        }
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
        let (vertex, fragment) = (ShaderStage::Vertex, ShaderStage::Fragment);
        let colour = [("vs_main", vertex), ("fs_main", fragment)];
        let motion = [
            ("vs_velocity", vertex),
            ("fs_velocity", fragment),       // #1435
            ("fs_velocity_ssr", fragment),   // #1526
            ("fs_depth_postpass", fragment), // #1523
        ];
        for (entry, stage) in colour.into_iter().chain(motion) {
            if !has(entry, stage) {
                return Err(format!("not a surface shader (no {entry})"));
            }
        }
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
#[path = "../tests/surface_tests.rs"]
mod surface_tests;

#[cfg(test)]
#[path = "../tests/params_tests.rs"]
mod params_tests;

#[cfg(test)]
#[path = "../tests/mask_tests.rs"]
pub(crate) mod mask_tests;

#[cfg(test)]
#[path = "../tests/gameplay_blocks_tests.rs"]
mod gameplay_blocks_tests;

#[cfg(test)]
#[path = "../tests/cut_depth_tests.rs"]
pub(crate) mod cut_depth_tests;

#[cfg(test)]
#[path = "../tests/priming/scroll_tests.rs"]
mod priming_scroll_tests;
#[cfg(test)]
#[path = "../tests/priming/walls_tests.rs"]
mod priming_tests;
