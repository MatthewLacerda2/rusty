//! `<name>.wgsl` resolves in `assets/shaders`, else `engine/shaders`.
//! [`STANDARD`] (0) is the renderer's own pair; `n > 0` is cache slot `n - 1`.
//! A module that is missing, lacks entries or fails validation is logged once and draws with the
//! standard pipeline. [`SurfaceShaders::refresh`] drains the write log once per render; a written
//! slot goes stale and rebuilds on its next draw. A variant that discards, moves UVs or vertices
//! builds [`CutPipelines`] reading the same params, so a dissolved fragment casts no shadow and
//! fills no SSAO depth.

mod pass;
mod upkeep;

use std::{collections::HashMap, path::Path};
use wgpu::naga::ShaderStage;

use rusty_core::fs::written::written_since;
use rusty_shadergen::assemble::{CUT_PREPASS, CUT_SHADOW};
use rusty_shadergen::params::{resolve_module, ParamLayout};
use rusty_shadergen::{baked, compose, engine_shader_dir, DEFAULT_OUT_DIR};

pub(crate) const STANDARD: usize = 0;

pub(crate) use pass::SolidPass;

type Built = (SurfacePipelines, ParamLayout);

pub(crate) struct SurfacePipelines {
    pub forward: super::FacedPipeline,
    pub velocity: super::VelocityPipelines,
    pub primed: super::PrimedPipelines,
    pub transparent: super::TransparentPipelines,
    pub depth_postpass: super::DepthPostpassPipelines,
    pub cut: Option<CutPipelines>,
}

pub(crate) struct CutPipelines {
    pub prepass: super::FacedPipeline,
    pub shadow: wgpu::RenderPipeline,
    pub mirrored_shadow: wgpu::RenderPipeline,
    pub two_sided_shadow: super::LazyPipeline,
}

struct Slot {
    name: String,
    pipelines: Option<SurfacePipelines>,
    params: ParamLayout,
    stale: bool,
}

pub(crate) struct SurfaceShaders {
    layout: wgpu::PipelineLayout,
    shadow_layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    dirs: Vec<String>,
    index: HashMap<String, usize>,
    slots: Vec<Slot>,
    builds: usize,
    seen: u64,
    warm: bool,
}

impl SurfaceShaders {
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

    #[cfg(test)]
    pub(crate) fn set_dirs(&mut self, dirs: Vec<String>) {
        self.dirs = dirs;
    }

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

    pub(crate) fn params(&self, id: usize) -> Option<&ParamLayout> {
        let slot = self.slots.get(id.checked_sub(1)?)?;
        (!slot.params.params.is_empty()).then_some(&slot.params)
    }

    #[cfg(test)]
    pub(crate) fn builds(&self) -> usize {
        self.builds
    }

    fn get(&self, id: usize) -> Option<&SurfacePipelines> {
        self.slots.get(id.checked_sub(1)?)?.pipelines.as_ref()
    }

    pub(crate) fn cut(&self, id: usize) -> Option<&CutPipelines> {
        self.get(id)?.cut.as_ref()
    }

    fn rebuild(&mut self, device: &wgpu::Device, i: usize) {
        let dirs: Vec<&str> = self.dirs.iter().map(String::as_str).collect();
        let module = resolve_module(&dirs, &self.slots[i].name);
        self.builds += 1;
        let built = match &module {
            Some(path) => self.build(device, path),
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

    fn build(&self, device: &wgpu::Device, path: &Path) -> Result<Built, String> {
        let baked = baked::load(path)?;
        let mut composer = compose::composer_with_common(engine_shader_dir())?;
        let module = compose::compose(&mut composer, &baked.source, &path.to_string_lossy())?;
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
            ("fs_velocity", fragment),
            ("fs_velocity_ssr", fragment),
            ("fs_depth_postpass", fragment),
            ("fs_transparent", fragment),
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
            None => Ok((pipelines, baked.layout(path)?)),
        }
    }
}

#[cfg(test)]
#[path = "../tests/cut_depth_tests.rs"]
pub(crate) mod cut_depth_tests;
#[cfg(test)]
#[path = "../tests/gameplay_blocks_tests.rs"]
mod gameplay_blocks_tests;
#[cfg(test)]
#[path = "../tests/mask_tests.rs"]
pub(crate) mod mask_tests;
#[cfg(test)]
#[path = "../tests/params_tests.rs"]
mod params_tests;
#[cfg(test)]
#[path = "../tests/priming/rank_tests.rs"]
mod priming_rank_tests;
#[cfg(test)]
#[path = "../tests/priming/scroll_tests.rs"]
mod priming_scroll_tests;
#[cfg(test)]
#[path = "../tests/priming/walls_tests.rs"]
mod priming_tests;
#[cfg(test)]
#[path = "../tests/rebake_tests.rs"]
mod rebake_tests;
#[cfg(test)]
#[path = "../tests/surface_tests.rs"]
mod surface_tests;
