mod effect;

use crate::gpu::ops::{CountedDevice as _, CountedQueue as _};
pub(crate) use effect::EFFECT;

use std::collections::{BTreeMap, HashMap};

use naga_oil::compose::Composer;

use crate::gpu::layout_entries::uniform_entry;
use crate::postfx::{PfxTarget, PostFx};
use rusty_core::fs::written::written_since;
use rusty_shadergen::params::{is_shader_file, PackedParams, ParamLayout};
use rusty_shadergen::{compose, engine_shader_dir, DEFAULT_OUT_DIR};

pub(crate) struct Effect {
    pub pipeline: wgpu::RenderPipeline,
    pub params: wgpu::BindGroup,
    layout: ParamLayout,
    buffer: wgpu::Buffer,
}

pub struct CustomChain {
    pub ldr_b: PfxTarget,
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

    pub(crate) fn prepare(&mut self, device: &wgpu::Device, names: &[String]) {
        self.effects.prepare(device, names);
    }

    pub(crate) fn loaded<'a>(&'a self, names: &'a [String]) -> Vec<&'a Effect> {
        let cache = &self.effects.pipelines;
        names
            .iter()
            .filter_map(|name| cache.get(name).and_then(Option::as_ref))
            .collect()
    }

    pub(crate) fn write_params(
        queue: &wgpu::Queue,
        effects: &[&Effect],
        values: &BTreeMap<String, Vec<f32>>,
    ) {
        for e in effects.iter().filter(|e| !e.layout.params.is_empty()) {
            let packed: PackedParams = e.layout.pack(values);
            queue.counted_write_buffer(&e.buffer, 0, bytemuck::cast_slice(&packed));
        }
    }
}

struct EffectCache {
    dir: String,
    layout: wgpu::PipelineLayout,
    params_layout: wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
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

    /// Traps wgpu's validation: a module that composes but doesn't fit the pass fails here, not
    /// on the device.
    fn load(&mut self, device: &wgpu::Device, name: &str) -> Result<Effect, String> {
        let path = format!("{}/{name}.wgsl", self.dir);
        let baked = rusty_shadergen::baked::load(std::path::Path::new(&path))?;
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
        let params = device.counted_bind_group(&wgpu::BindGroupDescriptor {
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
