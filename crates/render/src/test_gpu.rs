//! A test hook, with its submodules: how a test here, in `dev` or in `shell` gets a GPU
//! (`None`: skip).

use crate::Renderer;

pub fn headless_or_skip(width: u32, height: u32) -> Option<Renderer> {
    pollster::block_on(Renderer::new_headless(width, height))
}

#[cfg(any(test, feature = "dev"))]
pub mod culling;
pub mod pair;
#[cfg(any(test, feature = "dev"))]
pub mod ssil;
pub use pair::{headless_pair_or_skip, HeadlessPair};

#[cfg(test)]
pub(crate) fn black_sky(scene: &mut rusty_scene::Scene) {
    let path = rusty_test_support::temp::dir().join("black_sky.png");
    image::RgbaImage::from_pixel(2, 1, image::Rgba([0, 0, 0, 255]))
        .save(&path)
        .expect("write the black sky");
    scene.skybox_path = path.to_string_lossy().into_owned();
}

/// Runs `source`'s `main` (`@workgroup_size(1)`) on a 64×2 grid; returns binding 0's 256 `vec4`s.
#[cfg(test)]
pub(crate) fn run_probe(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source: wgpu::ShaderSource,
) -> Vec<[f32; 4]> {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let size = 256 * 16;
    let buffer = |usage| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let storage = buffer(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
    let staging = buffer(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: storage.as_entire_binding(),
        }],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(64, 2, 1);
    }
    encoder.copy_buffer_to_buffer(&storage, 0, &staging, 0, size);
    queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let mapped = slice.get_mapped_range().expect("mapped");
    bytemuck::cast_slice(&mapped).to_vec()
}

#[cfg(test)]
mod tests {
    #[test]
    fn gpu_adapter_present_when_required() {
        let renderer = super::headless_or_skip(4, 4);
        if let Some(r) = &renderer {
            let info = r.device.adapter_info();
            let driver = format!("{} {}", info.driver, info.driver_info);
            eprintln!(
                "[gpu] adapter: {} ({:?}, {driver})",
                info.name, info.backend
            );
        }
        if let Some(name) = crate::setup::headless::wanted_adapter() {
            let env = crate::ADAPTER_ENV;
            assert!(
                renderer.is_some(),
                "{env}={name} matches no adapter: fix or unset it"
            );
        }
        if !crate::gpu_required() {
            return;
        }
        assert!(
            renderer.is_some(),
            "{}=1 but no GPU or software adapter was found: on Linux install Mesa's \
             lavapipe (`mesa-vulkan-drivers`), or unset the variable to skip GPU tests",
            crate::REQUIRE_GPU_ENV
        );
    }
}
