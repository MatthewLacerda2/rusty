//! Authored post-FX effects (#397): the modules `Shader.Bake` writes with
//! `pass = "postfx"`, run in the order the active volume's `custom_effects` lists them.
//!
//! They run **after tonemapping, before FXAA**, on display-referred colour in `[0, 1]`
//! — where grades like vignette, scanlines and posterize are designed to work, and
//! before FXAA so an effect's own hard edges (scanlines, bands) are anti-aliased
//! like the scene's. Each effect is one fullscreen pass, ping-ponging between the
//! chain's two LDR targets.
//!
//! An effect's bind group is the chain's shared IO layout: binding 1 is the colour
//! so far, binding 2 its sampler, and binding 3 the **previous frame's** result of
//! this chain (black on the first frame and after a resize; only written while
//! effects run) — the history a feedback effect (#402) reads. Bindings 0, 4 and 5 are the post params, depth and
//! skybox, as for every other post pass.
//!
//! Modules are compiled once per name and cached; a successful bake anywhere in the
//! process drops the cache, so a re-baked effect is picked up next frame. A module
//! that is missing or fails to compile is logged once and skipped — never a crash,
//! never a black frame.

use std::collections::HashMap;

use naga_oil::compose::Composer;

use super::{PfxTarget, PostFx};
use crate::shadergen::{bake_generation, compose, DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

/// The authored-effect half of the chain: its extra targets and its module cache.
pub struct CustomChain {
    /// The second LDR ping-pong target (the first is [`PostFx::ldr`]).
    pub ldr_b: PfxTarget,
    /// Last frame's chain result, bound at slot 3 of every authored effect.
    pub history: PfxTarget,
    effects: EffectCache,
}

impl CustomChain {
    pub(super) fn new(
        device: &wgpu::Device,
        io_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        (width, height): (u32, u32),
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PostFX Custom Layout"),
            bind_group_layouts: &[Some(io_layout)],
            immediate_size: 0,
        });
        let (ldr_b, history) = Self::targets(device, format, width, height);
        Self {
            ldr_b,
            history,
            effects: EffectCache {
                dir: DEFAULT_OUT_DIR.to_string(),
                layout,
                format,
                generation: bake_generation(),
                composer: None,
                pipelines: HashMap::new(),
            },
        }
    }

    pub(super) fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        (self.ldr_b, self.history) = Self::targets(device, self.effects.format, width, height);
    }

    fn targets(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> (PfxTarget, PfxTarget) {
        let ldr_b = PostFx::target(device, width, height, "PostFX LDR B", format);
        let history = PostFx::target(device, width, height, "PostFX History", format);
        (ldr_b, history)
    }

    /// Compile any of `names` not yet cached (or dropped by a bake since).
    pub(super) fn prepare(&mut self, device: &wgpu::Device, names: &[String]) {
        self.effects.prepare(device, names);
    }

    /// The pipelines of `names` that loaded, in order — missing or broken modules
    /// are simply absent. Call [`prepare`](Self::prepare) first.
    pub(super) fn loaded<'a>(&'a self, names: &'a [String]) -> Vec<&'a wgpu::RenderPipeline> {
        let cache = &self.effects.pipelines;
        names
            .iter()
            .filter_map(|name| cache.get(name).and_then(Option::as_ref))
            .collect()
    }
}

/// Compiled authored modules by name. `None` records a failure, so it is logged once
/// rather than every frame, until the next bake invalidates the cache.
struct EffectCache {
    dir: String,
    layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    generation: u64,
    composer: Option<Composer>,
    pipelines: HashMap<String, Option<wgpu::RenderPipeline>>,
}

impl EffectCache {
    fn prepare(&mut self, device: &wgpu::Device, names: &[String]) {
        let generation = bake_generation();
        if generation != self.generation {
            self.generation = generation;
            self.pipelines.clear();
        }
        for name in names {
            if self.pipelines.contains_key(name) {
                continue;
            }
            let pipeline = self.load(device, name);
            if let Err(e) = &pipeline {
                log::warn!("post-FX effect `{name}` skipped: {e}");
            }
            self.pipelines.insert(name.clone(), pipeline.ok());
        }
    }

    /// Read, compose and build one effect's pipeline, trapping wgpu's validation so a
    /// module that composes but doesn't fit the pass fails here instead of panicking
    /// the device.
    fn load(&mut self, device: &wgpu::Device, name: &str) -> Result<wgpu::RenderPipeline, String> {
        let path = format!("{}/{name}.wgsl", self.dir);
        let source = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
        let composer = match &mut self.composer {
            Some(c) => c,
            slot => slot.insert(compose::composer_with_common(ENGINE_SHADER_DIR)?),
        };
        let module = compose::compose(composer, &source, &path)
            .map_err(|e| format!("{path} failed to compose: {e}"))?;
        for entry in ["vs_fullscreen", "fs_main"] {
            if !module.entry_points.iter().any(|e| e.name == entry) {
                return Err(format!("{path} has no `{entry}` (not a postfx module?)"));
            }
        }
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module)),
        });
        let pipeline = PostFx::fullscreen_pipeline(
            device,
            &shader,
            &self.layout,
            ("vs_fullscreen", "fs_main"),
            self.format,
        );
        match pollster::block_on(scope.pop()) {
            Some(e) => Err(format!("{path} does not fit the post pass: {e}")),
            None => Ok(pipeline),
        }
    }
}
