use std::sync::OnceLock;

use super::specs::{make_pipeline, PipelineSpec};

type Build<P> = Box<dyn Fn(&wgpu::Device) -> P + Send + Sync>;

pub(crate) struct LazyPipeline<P = wgpu::RenderPipeline> {
    cell: OnceLock<P>,
    device: wgpu::Device,
    build: Build<P>,
}

pub(crate) type LazyCompute = LazyPipeline<wgpu::ComputePipeline>;

impl<P> LazyPipeline<P> {
    pub(crate) fn new(
        device: &wgpu::Device,
        build: impl Fn(&wgpu::Device) -> P + Send + Sync + 'static,
    ) -> Self {
        Self {
            cell: OnceLock::new(),
            device: device.clone(),
            build: Box::new(build),
        }
    }

    pub(crate) fn get(&self) -> &P {
        self.cell.get_or_init(|| (self.build)(&self.device))
    }

    #[cfg(test)]
    pub(crate) fn is_built(&self) -> bool {
        self.cell.get().is_some()
    }
}

pub(super) fn lazy_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    spec: PipelineSpec,
) -> LazyPipeline {
    let (shader, layout) = (shader.clone(), layout.clone());
    LazyPipeline::new(device, move |device| {
        make_pipeline(device, &shader, format, &layout, &spec)
    })
}

pub(crate) fn lazy_compute(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    (label, entry): (&'static str, &'static str),
    groups: Option<&[&wgpu::BindGroupLayout]>,
) -> LazyCompute {
    let layout = groups.map(|groups| {
        let groups: Vec<_> = groups.iter().map(|g| Some(*g)).collect();
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: &groups,
            immediate_size: 0,
        })
    });
    let module = module.clone();
    LazyCompute::new(device, move |device| {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label),
            layout: layout.as_ref(),
            module: &module,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            cache: None,
        })
    })
}
