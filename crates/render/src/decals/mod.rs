mod atlas;
mod gpu;
mod kept;
mod record;

pub(crate) use gpu::DecalBuffers;
#[cfg(test)]
pub(crate) use record::GpuDecal;

#[cfg(test)]
mod fade_gpu_tests;
#[cfg(test)]
mod gpu_fixture;
#[cfg(test)]
mod gpu_tests;
