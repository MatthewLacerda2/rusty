//! src/render/test_gpu.rs — the one place a test asks for a GPU (#366).
//!
//! Every in-crate GPU test used to open with the same incantation:
//!
//! ```ignore
//! let Some(mut renderer) = pollster::block_on(Renderer::new_headless(64, 64)) else {
//!     return; // no adapter
//! };
//! ```
//!
//! Sixteen copies of a decision that has to be identical everywhere — *what counts as
//! "no GPU here", and what a test does about it* — is a rule with sixteen chances to
//! drift. It lives here instead.
//!
//! Integration tests (`tests/*.rs`) do not use this: they reach the renderer through
//! the dev layer (`screenshot::capture`, `Debug.Preview`, the probe bakes), which owns
//! its own skip handling (`dev::capture::no_adapter`, failing under the same variable,
//! #885). The budget in [`super::setup::budget`] covers both, because
//! it sits inside the constructor rather than in this helper.

use crate::render::Renderer;

use crate::render::REQUIRE_GPU_ENV;

/// A headless renderer, or `None` when this machine has no GPU or software adapter.
///
/// `None` means **skip, don't fail**, so a GPU-less machine still runs the rest of the
/// suite. CI never relies on that path: every CI job has an adapter (Metal on macOS,
/// WARP on Windows, Mesa's lavapipe on Linux, #489) and sets [`REQUIRE_GPU_ENV`], so
/// the canary below fails if the driver ever goes missing instead of letting every GPU
/// test quietly skip. A test that cannot tolerate being skipped locally does not belong
/// on the GPU — pin the rule it cares about with an adapter-free unit test as well.
///
/// The returned renderer holds a slot in the headless budget until it is dropped, so
/// callers should let it fall out of scope promptly rather than parking it in a
/// long-lived static.
pub(crate) fn headless_or_skip(width: u32, height: u32) -> Option<Renderer> {
    pollster::block_on(Renderer::new_headless(width, height))
}

/// Give `scene` a black skybox panorama, so nothing but its own lights and emissives
/// lights it. `ambient_intensity = 0` removes the ambient term only: the procedural
/// sky still lights every surface through its environment reflection (#1543), as
/// Unity's skybox does whatever its ambient multiplier says.
#[cfg(test)]
pub(crate) fn black_sky(scene: &mut crate::scene::Scene) {
    let path = crate::test_temp::dir().join("black_sky.png");
    image::RgbaImage::from_pixel(2, 1, image::Rgba([0, 0, 0, 255]))
        .save(&path)
        .expect("write the black sky");
    scene.skybox_path = path.to_string_lossy().into_owned();
}

/// A compute probe: dispatch `source`'s `main` over 64 × 2 invocations and read
/// back the 256 `vec4`s it writes to its one binding, a storage array (#1524): how
/// a test runs a WGSL helper as written on the GPU.
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
    /// The one GPU test that refuses to skip, and only where asked to. Without it, a
    /// CI runner that lost its software driver would put every GPU test back on the
    /// silent `None` path and still report green — the state #489 fixed.
    #[test]
    fn gpu_adapter_present_when_required() {
        if !crate::render::gpu_required() {
            return;
        }
        assert!(
            super::headless_or_skip(4, 4).is_some(),
            "{}=1 but no GPU or software adapter was found: on Linux install Mesa's \
             lavapipe (`mesa-vulkan-drivers`), or unset the variable to skip GPU tests",
            super::REQUIRE_GPU_ENV
        );
    }
}
