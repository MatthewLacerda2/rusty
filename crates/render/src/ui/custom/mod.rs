use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) mod shaded;
mod uniforms;

pub(crate) use uniforms::ShadeUniforms;
use uniforms::SLOT;

use super::cache::UiViewCache;
use super::pipeline::{self, UiPass};
use crate::gpu::{layout_entries::dynamic_uniform_entry, ops::CountedDevice as _};
use rusty_components::UiBlend;
use rusty_core::fs::written::written_since;
use rusty_shadergen::baked::{self, Baked};
use rusty_shadergen::params::{self, ParamLayout};
use rusty_shadergen::{compose, engine_shader_dir, DEFAULT_OUT_DIR};

struct Variant {
    module: wgpu::ShaderModule,
    params: ParamLayout,
    world: Vec<wgpu::RenderPipeline>,
    screen: HashMap<(wgpu::TextureFormat, UiBlend), wgpu::RenderPipeline>,
}

type Source = Option<(PathBuf, Baked)>;

fn source(name: &str) -> Source {
    let path = params::resolve_module(&[DEFAULT_OUT_DIR, engine_shader_dir()], name)?;
    let baked = baked::load(&path).ok()?;
    Some((path, baked))
}

pub(crate) struct UiShaders {
    shade_layout: wgpu::BindGroupLayout,
    empty: wgpu::BindGroup,
    screen_layout: wgpu::PipelineLayout,
    world_layout: wgpu::PipelineLayout,
    seen: u64,
    variants: HashMap<String, (Source, Option<Variant>)>,
    builds: usize,
}

impl UiShaders {
    pub(crate) fn new(
        device: &wgpu::Device,
        [texture, batch, world]: [&wgpu::BindGroupLayout; 3],
    ) -> Self {
        let shade_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI Shade Layout"),
            entries: &[dynamic_uniform_entry(0, wgpu::ShaderStages::FRAGMENT, SLOT)],
        });
        let empty_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI Empty Layout"),
            entries: &[],
        });
        let empty = device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Empty Group"),
            layout: &empty_layout,
            entries: &[],
        });
        let layout = |label, groups: &[&wgpu::BindGroupLayout]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &groups.iter().copied().map(Some).collect::<Vec<_>>(),
                immediate_size: 0,
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
            seen: written_since(u64::MAX).0,
            variants: HashMap::new(),
            builds: 0,
        }
    }

    fn variant(&mut self, device: &wgpu::Device, name: &str) -> Option<&Variant> {
        if !self.variants.contains_key(name) {
            self.builds += 1;
            let source = source(name);
            let built = self.build(device, name, &source).map_err(|e| {
                log::warn!("[Render] ui shader {name:?}: {e}; drawing with the standard UI shader");
            });
            self.variants.insert(name.to_owned(), (source, built.ok()));
        }
        self.variants.get(name)?.1.as_ref()
    }

    fn build(&self, device: &wgpu::Device, name: &str, source: &Source) -> Result<Variant, String> {
        let engine = engine_shader_dir();
        let (path, baked) = source
            .as_ref()
            .ok_or_else(|| format!("no `{name}.wgsl` in {DEFAULT_OUT_DIR} or {engine}"))?;
        let mut composer = compose::composer_with_common(engine)?;
        let module = compose::compose(&mut composer, &baked.source, &path.to_string_lossy())?;
        for entry in ["vs_main", "fs_main", "vs_world", "fs_world"] {
            if !module.entry_points.iter().any(|e| e.name == entry) {
                return Err(format!(
                    "not a ui shader (no {entry}); bake it with pass \"ui\""
                ));
            }
        }
        let params = baked.layout(path)?;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Variant Shader"),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module)),
        });
        let shader = ("UI Variant World", &module, &self.world_layout);
        let world = UiBlend::ALL
            .map(|mode| pipeline::build(device, shader, UiPass::World, mode))
            .into();
        match pollster::block_on(scope.pop()) {
            Some(e) => Err(e.to_string()),
            None => Ok(Variant {
                module,
                params,
                world,
                screen: HashMap::new(),
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn builds(&self) -> usize {
        self.builds
    }

    #[cfg(test)]
    pub(crate) fn built(&self, name: &str) -> bool {
        self.variants.get(name).is_some_and(|(_, v)| v.is_some())
    }

    fn refresh(&mut self) {
        let (seen, paths) = written_since(self.seen);
        self.seen = seen;
        if paths.iter().any(|p| params::is_shader_file(p)) {
            self.variants
                .retain(|name, (built_from, _)| *built_from == source(name));
        }
    }

    pub(super) fn ensure_screen(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        for variant in self.variants.values_mut().filter_map(|(_, v)| v.as_mut()) {
            for mode in UiBlend::ALL {
                if !variant.screen.contains_key(&(format, mode)) {
                    let shader = ("UI Variant", &variant.module, &self.screen_layout);
                    let built = pipeline::build(device, shader, UiPass::Screen(format), mode);
                    variant.screen.insert((format, mode), built);
                }
            }
        }
    }

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
                .1
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
            rp.set_bind_group(pipeline::CANVAS_GROUP, &self.empty, &[]);
        }
        rp.set_bind_group(pipeline::SHADE_GROUP, group, &[offset]);
        true
    }
}

impl super::UiRenderer {
    /// `bound` holds the (blend, backdrop) of the standard pipeline last set, `None` after a
    /// custom one. Returns `false` when no pipeline is prepared for the format: skip the batch.
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
            (UiPass::World, _) => self.world.pipeline(mode.0),
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
#[path = "gpu_bake_tests.rs"]
mod gpu_bake_tests;

#[cfg(test)]
#[path = "gpu_draw_tests.rs"]
mod gpu_draw_tests;
