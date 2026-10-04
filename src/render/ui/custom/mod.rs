//! src/render/ui/custom/ — custom ui shaders on the GPU (#427): Unity's
//! `Graphic.material`.
//!
//! A graphic naming a baked ui shader (`Shader.Bake` with `pass = "ui"`) draws its
//! batch with that variant's pipeline instead of the standard UI one. A variant is
//! `ui.wgsl` with `graphic()` replaced (`shadergen::assemble::ui`), so it binds the
//! standard groups unchanged — 0 the batch's texture, 1 its clip, feather and Mask
//! coverage (#428), and on world canvases 2 the placement — and a custom-shaded
//! graphic is clipped and masked exactly like any other. It adds group 3: the
//! per-graphic `UiShade` uniform — its rect, the UI clock and its runtime params
//! (#399's packing). The screen pass binds an empty group 2.
//!
//! **Cache, mirroring the surface one (#626).** Variants are keyed by shader name,
//! built the first frame a graphic names one: composed against `common`, checked for
//! all four UI entry points, then built under a validation error scope. A module
//! that is missing, fails to compose, is not a ui module, or fails wgpu validation is
//! logged **once** and its graphics draw with the standard shader — never a crash.
//! A bake anywhere in the process (`shadergen::bake_generation`) drops the cache, so
//! a re-baked shader is picked up next frame.
//!
//! The per-graphic uniforms each view packs every frame are in `uniforms`.

use std::collections::HashMap;

pub(crate) mod shaded;
mod uniforms;

pub(crate) use uniforms::ShadeUniforms;
use uniforms::SLOT;

use super::cache::UiViewCache;
use super::pipeline::{self, UiPass};
use crate::components::UiBlend;
use crate::shadergen::params::{self, ParamLayout};
use crate::shadergen::{bake_generation, compose, DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

/// One built variant.
struct Variant {
    module: wgpu::ShaderModule,
    params: ParamLayout,
    /// One per blend mode (#425), in `UiBlend::ALL` order.
    world: Vec<wgpu::RenderPipeline>,
    screen: HashMap<(wgpu::TextureFormat, UiBlend), wgpu::RenderPipeline>,
}

/// The variant cache and the layouts its pipelines share. One per `UiRenderer`.
pub(crate) struct UiShaders {
    shade_layout: wgpu::BindGroupLayout,
    /// Bound at group 1 by the screen pass, which has no world placement.
    empty: wgpu::BindGroup,
    screen_layout: wgpu::PipelineLayout,
    world_layout: wgpu::PipelineLayout,
    generation: u64,
    /// `None` records a failure, logged once until the next bake.
    variants: HashMap<String, Option<Variant>>,
    /// Variant builds so far — a runtime param change must never add one.
    builds: usize,
}

impl UiShaders {
    /// An empty cache whose variants bind `texture` at group 0 and, in the world
    /// pass, `world` at group 1.
    pub(crate) fn new(
        device: &wgpu::Device,
        [texture, batch, world]: [&wgpu::BindGroupLayout; 3],
    ) -> Self {
        let shade_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI Shade Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(SLOT),
                },
                count: None,
            }],
        });
        let empty_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI Empty Layout"),
            entries: &[],
        });
        let empty = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Empty Group"),
            layout: &empty_layout,
            entries: &[],
        });
        let layout = |label, groups: &[&wgpu::BindGroupLayout]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: groups,
                push_constant_ranges: &[],
            })
        };
        Self {
            screen_layout: layout(
                "UI Custom Layout",
                &[texture, batch, &empty_layout, &shade_layout],
            ),
            world_layout: layout(
                "World UI Custom Layout",
                &[texture, batch, world, &shade_layout],
            ),
            shade_layout,
            empty,
            generation: bake_generation(),
            variants: HashMap::new(),
            builds: 0,
        }
    }

    /// The variant named `name`, building it on first use; `None` when it failed.
    fn variant(&mut self, device: &wgpu::Device, name: &str) -> Option<&Variant> {
        if !self.variants.contains_key(name) {
            self.builds += 1;
            let built = self.build(device, name).map_err(|e| {
                log::warn!("ui shader {name:?}: {e}; drawing with the standard UI shader");
            });
            self.variants.insert(name.to_owned(), built.ok());
        }
        self.variants.get(name)?.as_ref()
    }

    /// Compose `name`'s module, check it is a ui module, and build its world
    /// pipeline under a validation error scope, so wgpu refuses instead of panicking.
    fn build(&self, device: &wgpu::Device, name: &str) -> Result<Variant, String> {
        let dirs = [DEFAULT_OUT_DIR, ENGINE_SHADER_DIR];
        let path = params::resolve_module(&dirs, name)
            .ok_or_else(|| format!("no `{name}.wgsl` in {}", dirs.join(" or ")))?;
        let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut composer = compose::composer_with_common(ENGINE_SHADER_DIR)?;
        let module = compose::compose(&mut composer, &source, &path.to_string_lossy())?;
        for entry in ["vs_main", "fs_main", "vs_world", "fs_world"] {
            if !module.entry_points.iter().any(|e| e.name == entry) {
                return Err(format!(
                    "not a ui shader (no {entry}); bake it with pass \"ui\""
                ));
            }
        }
        let params = ParamLayout::read_beside(&path)?;
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Variant Shader"),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module)),
        });
        let shader = ("UI Variant World", &module, &self.world_layout);
        let world = UiBlend::ALL
            .map(|mode| pipeline::build(device, shader, UiPass::World, mode))
            .into();
        match pollster::block_on(device.pop_error_scope()) {
            Some(e) => Err(e.to_string()),
            None => Ok(Variant {
                module,
                params,
                world,
                screen: HashMap::new(),
            }),
        }
    }

    /// Variant builds so far (each attempt, failed or not, counts one).
    #[cfg(test)]
    pub(crate) fn builds(&self) -> usize {
        self.builds
    }

    /// Whether `name`'s variant is cached and built — `false` before its first frame
    /// and after a failure, when its graphics fell back to the standard shader.
    #[cfg(test)]
    pub(crate) fn built(&self, name: &str) -> bool {
        self.variants.get(name).is_some_and(Option::is_some)
    }

    /// Drop every variant when a bake happened since they were built.
    fn refresh(&mut self) {
        let generation = bake_generation();
        if generation != self.generation {
            self.generation = generation;
            self.variants.clear();
        }
    }

    /// Build every loaded variant's screen pipelines for `format` if absent, one per
    /// blend mode.
    pub(super) fn ensure_screen(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        for variant in self.variants.values_mut().flatten() {
            for mode in UiBlend::ALL {
                if !variant.screen.contains_key(&(format, mode)) {
                    let shader = ("UI Variant", &variant.module, &self.screen_layout);
                    let built = pipeline::build(device, shader, UiPass::Screen(format), mode);
                    variant.screen.insert((format, mode), built);
                }
            }
        }
    }

    /// Bind batch `b` of canvas `c`'s custom ui shader for `pass` — its pipeline (in
    /// the batch's blend mode) and uniform — when it names one whose variant built,
    /// returning whether it did; the caller binds the standard pipeline otherwise.
    /// The screen pass binds an empty group 2; the world pass has bound its placement
    /// there. Group 1 (the batch's clip and mask, #428) is the caller's, as for any
    /// batch, so a custom-shaded graphic is clipped and masked like every other.
    pub(super) fn bind<'a>(
        &'a self,
        rp: &mut wgpu::RenderPass<'a>,
        (cache, c, b): (&'a UiViewCache, usize, usize),
        pass: UiPass,
    ) -> bool {
        let custom = || {
            let uniforms = cache.shades.as_ref()?;
            let offset = *uniforms.offsets.get(&(c, b))?;
            let batch = &cache.canvas(c).mesh.batches[b];
            let variant = self
                .variants
                .get(&batch.shade.as_ref()?.shader.name)?
                .as_ref()?;
            let pipeline = match pass {
                UiPass::Screen(format) => variant.screen.get(&(format, batch.blend))?,
                UiPass::World => &variant.world[super::blend::index(batch.blend)],
            };
            Some((pipeline, &uniforms.group, offset))
        };
        let Some((pipeline, group, offset)) = custom() else {
            return false;
        };
        rp.set_pipeline(pipeline);
        if matches!(pass, UiPass::Screen(_)) {
            rp.set_bind_group(2, &self.empty, &[]);
        }
        rp.set_bind_group(3, group, &[offset]);
        true
    }
}

impl super::UiRenderer {
    /// Bind the pipeline batch `b` of canvas `c` draws with in `pass`: its custom ui
    /// shader's when it has one that built, else the standard one — a backdrop's
    /// (#426, always "over") or its blend mode's — skipped when `bound` (blend mode,
    /// backdrop) says that one is bound already. Returns `false` when there is no
    /// pipeline to draw with (a screen format not prepared).
    pub(super) fn bind_batch<'a>(
        &'a self,
        rp: &mut wgpu::RenderPass<'a>,
        at: (&'a UiViewCache, usize, usize),
        pass: UiPass,
        bound: &mut Option<(UiBlend, bool)>,
    ) -> bool {
        if self.shaders.bind(rp, at, pass) {
            *bound = None;
            return true;
        }
        let batch = &at.0.canvas(at.1).mesh.batches[at.2];
        let mode = (batch.blend, batch.backdrop.is_some());
        if *bound == Some(mode) {
            return true;
        }
        let standard = match (pass, mode.1) {
            (UiPass::Screen(format), true) => self.backdrops.get(&format),
            (UiPass::Screen(format), false) => self.pipelines.get(&(format, mode.0)),
            (UiPass::World, _) => self.world.pipelines.get(super::blend::index(mode.0)),
        };
        let Some(pipeline) = standard else {
            return false;
        };
        rp.set_pipeline(pipeline);
        *bound = Some(mode);
        true
    }
}

#[cfg(test)]
mod fixture;

#[cfg(test)]
#[path = "blocks_tests.rs"]
mod blocks_tests;

#[cfg(test)]
#[path = "gpu_tests.rs"]
mod gpu_tests;
