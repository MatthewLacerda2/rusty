use crate::Renderer;

pub const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub const REQUIRE_GPU_ENV: &str = "RUSTY_REQUIRE_GPU";

pub fn gpu_required() -> bool {
    std::env::var(REQUIRE_GPU_ENV).as_deref() == Ok("1")
}

pub const ADAPTER_ENV: &str = "WGPU_ADAPTER_NAME";

pub(crate) fn wanted_adapter() -> Option<String> {
    let name = std::env::var(ADAPTER_ENV).ok()?.trim().to_lowercase();
    (!name.is_empty()).then_some(name)
}

fn is_wanted(name: &str, wanted: &str) -> bool {
    name.to_lowercase().contains(wanted)
}

/// Never GL: its EGL display is process-wide, and the renderers sharing `instance` would race on
/// it.
pub(crate) const HEADLESS_BACKENDS: wgpu::Backends = wgpu::Backends::PRIMARY;

/// One for the process, never dropped: NVIDIA's Linux driver lists no adapter after about 19
/// instances made and freed.
fn instance() -> &'static wgpu::Instance {
    static INSTANCE: std::sync::OnceLock<wgpu::Instance> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| {
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: HEADLESS_BACKENDS,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        })
    })
}

impl Renderer {
    pub async fn new_headless(width: u32, height: u32) -> Option<Self> {
        let permit = super::budget::acquire_headless();
        Self::new_headless_with(permit, width, height).await
    }

    pub(crate) async fn new_headless_with(
        permit: super::budget::Permit,
        width: u32,
        height: u32,
    ) -> Option<Self> {
        let width = width.max(1);
        let height = height.max(1);
        let (device, queue) = device().await?;

        // No real swapchain offscreen: only the always-available `Fifo` mode.
        Some(Self::from_parts(
            device,
            queue,
            None,
            offscreen_config(width, height),
            vec![wgpu::PresentMode::Fifo],
            Some(permit),
        ))
    }
}

async fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let adapter = pick_adapter(instance()).await?;
    let wanted = wgpu::Features::TIMESTAMP_QUERY | super::limits::OPTIONAL;
    let features = adapter.features() & wanted;
    let descriptor = wgpu::DeviceDescriptor {
        required_features: features,
        required_limits: super::limits::device_limits(),
        label: Some("Headless Screenshot Device"),
        ..Default::default()
    };
    adapter.request_device(&descriptor).await.ok()
}

async fn pick_adapter(instance: &wgpu::Instance) -> Option<wgpu::Adapter> {
    if let Some(wanted) = wanted_adapter() {
        let adapters = instance.enumerate_adapters(HEADLESS_BACKENDS).await;
        let found = (adapters.into_iter()).find(|a| is_wanted(&a.get_info().name, &wanted));
        if found.is_none() {
            log::warn!("[Render] no adapter matches {ADAPTER_ENV}={wanted}");
        }
        return found;
    }
    let options = |power_preference, force_fallback_adapter| wgpu::RequestAdapterOptions {
        power_preference,
        compatible_surface: None,
        force_fallback_adapter,
        apply_limit_buckets: false,
    };
    let high = options(wgpu::PowerPreference::HighPerformance, false);
    match instance.request_adapter(&high).await {
        Ok(a) => Some(a),
        Err(_) => (instance)
            .request_adapter(&options(wgpu::PowerPreference::LowPower, true))
            .await
            .ok(),
    }
}

fn offscreen_config(width: u32, height: u32) -> wgpu::SurfaceConfiguration {
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: OFFSCREEN_FORMAT,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        color_space: wgpu::SurfaceColorSpace::Auto,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_wanted, HEADLESS_BACKENDS};

    #[test]
    fn gpu_more_headless_devices_than_the_driver_cap_each_find_an_adapter_in_one_process() {
        const DEVICES: usize = 32;
        let lost = |i: usize| format!("device {i} of {DEVICES} found no adapter");
        {
            let _permit = crate::setup::budget::acquire_headless();
            for i in 0..DEVICES - 1 {
                if pollster::block_on(super::device()).is_some() {
                    continue;
                }
                let required = crate::gpu_required() || super::wanted_adapter().is_some();
                assert!(i == 0 && !required, "{}", lost(i));
                return;
            }
        }
        let mut renderer = crate::test_gpu::headless_or_skip(4, 4)
            .unwrap_or_else(|| panic!("{}", lost(DEVICES - 1)));
        let (scene, cam) = (
            rusty_scene::Scene::new(),
            rusty_scene::Camera::new(glam::Vec3::ZERO, -90.0, 0.0),
        );
        let mut view =
            crate::RenderView::offscreen(&renderer.device, super::OFFSCREEN_FORMAT, 4, 4);
        let out = view.color_target_view().unwrap();
        renderer.render(&mut view, &scene, &cam, &out, false);
        let texture = view.color_target().unwrap();
        let px =
            crate::readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, 4, 4);
        assert!(
            px.chunks(4).all(|p| p[3] == 255),
            "the last device drew no opaque frame"
        );
    }

    #[test]
    fn an_adapter_is_picked_by_any_part_of_its_name_in_any_case() {
        let lavapipe = "llvmpipe (LLVM 20.1.8, 256 bits)";
        assert!(is_wanted(lavapipe, "llvmpipe"));
        assert!(is_wanted("NVIDIA GeForce RTX 2060", "rtx 2060"));
        assert!(!is_wanted(lavapipe, "nvidia"));
    }

    #[test]
    fn headless_renderers_never_bring_up_the_gl_backend() {
        assert!(!HEADLESS_BACKENDS.contains(wgpu::Backends::GL));
        for backend in [
            wgpu::Backends::VULKAN,
            wgpu::Backends::METAL,
            wgpu::Backends::DX12,
        ] {
            assert!(HEADLESS_BACKENDS.contains(backend), "{backend:?}");
        }
    }
}

#[cfg(test)]
#[path = "colour_space_tests.rs"]
mod colour_space_tests;
