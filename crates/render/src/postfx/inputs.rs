use crate::timing::GpuTimer;

pub struct PostFxContext<'a> {
    pub depth_view: &'a wgpu::TextureView,
    pub skybox: (&'a wgpu::TextureView, &'a wgpu::Sampler),
    pub output: &'a wgpu::TextureView,
    pub(crate) timer: &'a GpuTimer,
    pub(crate) passes: &'a super::effects::Passes,
}
