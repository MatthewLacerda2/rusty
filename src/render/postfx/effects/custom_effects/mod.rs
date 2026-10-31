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
//! effects run) — the history a feedback effect (#402) reads. Bindings 0, 4, 5 and 6 are the post params, depth,
//! skybox and the skybox's sampler, as for every other post pass. Group 1 is the effect's own **runtime-param
//! uniform** (#671, `shadergen::params::POSTFX_UNIFORM_DECL`): packed every frame from
//! the volume's `custom_effects.params` against the module's baked layout, so a script changing
//! a value costs one buffer write — never a re-bake or a pipeline rebuild.
//!
//! Modules are compiled once per name and cached; a shader file written anywhere in
//! the process (the write log, `core::fs::written`: a bake, or an outside edit the
//! editor's file watch noted, #1327) drops the cache, so a re-baked effect is
//! picked up next frame. A module
//! that is missing or fails to compile is logged once and skipped — never a crash,
//! never a black frame.

mod effect;

pub(crate) use effect::EFFECT;

use std::collections::{BTreeMap, HashMap};

use naga_oil::compose::Composer;

use crate::core::fs::written::written_since;
use crate::render::gpu::layout_entries::uniform_entry;
use crate::render::postfx::{PfxTarget, PostFx};
use crate::shadergen::params::{is_shader_file, PackedParams, ParamLayout};
use crate::shadergen::{compose, engine_shader_dir, DEFAULT_OUT_DIR};

/// One loaded authored effect: its pipeline, its runtime-param layout, and the
/// uniform buffer + group-1 bind group that layout is packed into.
pub(crate) struct Effect {
    pub pipeline: wgpu::RenderPipeline,
    pub params: wgpu::BindGroup,
    layout: ParamLayout,
    buffer: wgpu::Buffer,
}

/// The authored-effect half of the chain: its extra targets and its module cache.
pub struct CustomChain {
    /// The second LDR ping-pong target (the first is [`PostFx::ldr`]).
    pub ldr_b: PfxTarget,
    /// Last frame's chain result, bound at slot 3 of every authored effect.
    pub history: PfxTarget,
    effects: EffectCache,
}

impl CustomChain {
    pub(crate) fn new(
        device: &wgpu::Device,
        io_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        (width, height): (u32, u32),
    ) -> Self {
        let params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PostFX Custom Params Layout"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::FRAGMENT)],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PostFX Custom Layout"),
            bind_group_layouts: &[Some(io_layout), Some(&params_layout)],
            immediate_size: 0,
        });
        let (ldr_b, history) = Self::targets(device, format, width, height);
        Self {
            ldr_b,
            history,
            effects: EffectCache {
                dir: DEFAULT_OUT_DIR.to_string(),
                layout,
                params_layout,
                format,
                seen: written_since(u64::MAX).0,
                composer: None,
                pipelines: HashMap::new(),
            },
        }
    }

    pub(crate) fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
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
    pub(crate) fn prepare(&mut self, device: &wgpu::Device, names: &[String]) {
        self.effects.prepare(device, names);
    }

    /// The effects of `names` that loaded, in order — missing or broken modules are
    /// simply absent. Call [`prepare`](Self::prepare) first.
    pub(crate) fn loaded<'a>(&'a self, names: &'a [String]) -> Vec<&'a Effect> {
        let cache = &self.effects.pipelines;
        names
            .iter()
            .filter_map(|name| cache.get(name).and_then(Option::as_ref))
            .collect()
    }

    /// Pack each of `effects`' runtime params from the volume's `values` (its
    /// defaults where a value is unset) into its uniform.
    pub(crate) fn write_params(
        queue: &wgpu::Queue,
        effects: &[&Effect],
        values: &BTreeMap<String, Vec<f32>>,
    ) {
        for e in effects.iter().filter(|e| !e.layout.params.is_empty()) {
            let packed: PackedParams = e.layout.pack(values);
            queue.write_buffer(&e.buffer, 0, bytemuck::cast_slice(&packed));
        }
    }
}

/// Compiled authored modules by name. `None` records a failure, so it is logged once
/// rather than every frame, until a written shader file invalidates the cache.
struct EffectCache {
    dir: String,
    layout: wgpu::PipelineLayout,
    params_layout: wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    /// The newest write-log generation already looked at.
    seen: u64,
    composer: Option<Composer>,
    pipelines: HashMap<String, Option<Effect>>,
}

impl EffectCache {
    fn prepare(&mut self, device: &wgpu::Device, names: &[String]) {
        let (seen, written) = written_since(self.seen);
        self.seen = seen;
        if written.iter().any(|p| is_shader_file(p)) {
            self.pipelines.clear();
        }
        for name in names {
            if self.pipelines.contains_key(name) {
                continue;
            }
            let pipeline = self.load(device, name);
            if let Err(e) = &pipeline {
                log::warn!("[Render] post-FX effect `{name}` skipped: {e}");
            }
            self.pipelines.insert(name.clone(), pipeline.ok());
        }
    }

    /// Read, compose and build one effect's pipeline and param uniform, trapping
    /// wgpu's validation so a module that composes but doesn't fit the pass fails
    /// here instead of panicking the device.
    fn load(&mut self, device: &wgpu::Device, name: &str) -> Result<Effect, String> {
        let path = format!("{}/{name}.wgsl", self.dir);
        // Stale against the engine's shaders: brought up to date first (#1638).
        let baked = crate::shadergen::baked::load(std::path::Path::new(&path))?;
        let layout = baked.layout(std::path::Path::new(&path))?;
        let composer = match &mut self.composer {
            Some(c) => c,
            slot => slot.insert(compose::composer_with_common(engine_shader_dir())?),
        };
        let module = compose::compose(composer, &baked.source, &path)
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
            None => Ok(self.effect(device, name, pipeline, layout)),
        }
    }

    /// Wrap `pipeline` with its param uniform, seeded with the baked defaults.
    fn effect(
        &self,
        device: &wgpu::Device,
        name: &str,
        pipeline: wgpu::RenderPipeline,
        layout: ParamLayout,
    ) -> Effect {
        use wgpu::util::DeviceExt;
        let packed: PackedParams = layout.pack(&BTreeMap::new());
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(name),
            contents: bytemuck::cast_slice(&packed),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let params = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(name),
            layout: &self.params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Effect {
            pipeline,
            params,
            layout,
            buffer,
        }
    }
}
