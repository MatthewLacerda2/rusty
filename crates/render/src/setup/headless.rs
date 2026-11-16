//! crates/render/src/setup/headless.rs — Offscreen (no-window) renderer construction.
//!
//! Builds a fully functional `Renderer` with NO window surface, for the dev-layer
//! screenshot path. Reuses `Renderer::from_parts` so the offscreen renderer is
//! byte-identical to the windowed one apart from `surface == None`.
//!
//! GPU availability is NOT guaranteed in CI/containers, so adapter and device
//! acquisition return `None` (never panic) when no GPU or software adapter (e.g.
//! lavapipe) is present — the caller logs and skips gracefully.

use crate::Renderer;

/// The colour format the offscreen target renders into. sRGB, like the window
/// surface: the target format owns the one linear→display encode (#415), so the
/// copied-back bytes are already display-encoded PNG channel values and a headless
/// screenshot matches the window.
pub const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Set to `1` where a missing adapter is a failure, not a skip — CI sets it (#489).
/// Read through [`gpu_required`]; the in-crate canary and the dev capture layer both
/// answer to it (#885).
pub const REQUIRE_GPU_ENV: &str = "RUSTY_REQUIRE_GPU";

/// Whether this process was told a GPU or software adapter must be present
/// ([`REQUIRE_GPU_ENV`] is `1`). Without it, "no adapter" stays a graceful skip.
pub fn gpu_required() -> bool {
    std::env::var(REQUIRE_GPU_ENV).as_deref() == Ok("1")
}

/// Set to an adapter's name, or part of it (case ignored), to put every headless
/// renderer on that adapter alone: `llvmpipe` for lavapipe beside a real GPU, or
/// `nvidia`, so one machine tells a change from its driver (`make test-gpu`). It is
/// wgpu's own variable (`wgpu::util::initialize_adapter_from_env`), matched the same
/// way. No adapter of that name is "no adapter": a skip for each test, and a failure
/// of the adapter canary (`render::test_gpu`) whatever [`REQUIRE_GPU_ENV`] says, since
/// a name asked for is an adapter required. Unset or empty, the usual pick stands. It
/// steers every headless renderer (the bakes, `dev::capture`, `Debug.Preview`), never
/// the windowed one.
pub const ADAPTER_ENV: &str = "WGPU_ADAPTER_NAME";

/// The adapter [`ADAPTER_ENV`] asks for, lowercased, when it asks for one.
pub(crate) fn wanted_adapter() -> Option<String> {
    let name = std::env::var(ADAPTER_ENV).ok()?.trim().to_lowercase();
    (!name.is_empty()).then_some(name)
}

/// Whether an adapter called `name` is the one asked for (`wanted`, lowercased).
fn is_wanted(name: &str, wanted: &str) -> bool {
    name.to_lowercase().contains(wanted)
}

/// The backends a headless renderer may use: Vulkan, Metal and DX12 — never GL.
///
/// Every headless renderer in a process shares one `wgpu::Instance` (`instance`);
/// each still requests its own adapter and device. Leaving GL out is what makes that
/// sharing safe: on Linux, `Backends::all()` also brings up wgpu-hal's GLES/EGL
/// backend, whose EGL display is process-wide state that several live renderers race
/// on (#518). Bringing GL back would need that revisited. Every adapter a headless
/// render runs on — a real GPU, lavapipe on Linux CI, Metal on macOS — is a primary
/// backend, so leaving GL out costs no coverage. The windowed renderer keeps `all()`:
/// it owns one instance per process, and GL-only machines need it.
pub(crate) const HEADLESS_BACKENDS: wgpu::Backends = wgpu::Backends::PRIMARY;

/// The one `wgpu::Instance` every headless renderer in this process requests its
/// adapter from, made on first use and never dropped.
///
/// One instance per renderer meant, per renderer, one Vulkan instance created and
/// destroyed and, where no other instance held the loader, one load and unload of
/// `libvulkan` and its drivers (wgpu-hal's Vulkan instance owns its `ash::Entry`).
/// NVIDIA's Linux driver stops listing any adapter after about nineteen of those in
/// one process, so the twentieth bake of an editor session skipped (#2236); which of
/// the two trips it is not known, and the shared instance removes both. An instance
/// holds no per-renderer state — the adapter, device and queue do — so sharing it
/// changes nothing a renderer sees. The adapters it can see are fixed when it is first
/// made, so a driver or ICD installed later shows only after a restart.
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
    /// Build a headless renderer at `width` x `height`. Returns `None` (after a
    /// clear log) when no GPU/software adapter or device is available, so neither
    /// `cargo build` nor CI ever depends on a GPU at runtime.
    pub async fn new_headless(width: u32, height: u32) -> Option<Self> {
        // Claim a slot in the headless budget *before* allocating anything, and hold it
        // for this renderer's whole life (#366). Every headless consumer — the tests,
        // the screenshot path, the probe/reflection bakes, `Debug.Preview` — funnels
        // through here, so this is the one place the bound can be enforced for all of
        // them. Claimed before the adapter/device requests (`new_headless_with`), so
        // the count bounds live allocations; if one of those `?`s bails out, the permit
        // drops with this frame and the slot is never leaked.
        let permit = super::budget::acquire_headless();
        Self::new_headless_with(permit, width, height).await
    }

    /// [`new_headless`](Self::new_headless) on a slot the caller already holds: one
    /// share of a [`Pair`](super::budget::Pair), for a test that keeps two renderers
    /// alive at once (`render::test_gpu::headless_pair_or_skip`, #1844).
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

/// A headless renderer's device and queue, on the adapter [`pick_adapter`] finds in
/// the shared [`instance`], or `None` when there is no adapter or it refuses a device.
async fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let adapter = pick_adapter(instance()).await?;
    // Timestamps where the adapter has them: the frame stats' GPU time (#835).
    // And the rest of `limits::OPTIONAL`.
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

/// The adapter a headless renderer runs on: the one [`ADAPTER_ENV`] names, else the
/// high-performance one, else a software fallback (lavapipe), so headless containers
/// still render. No `compatible_surface`: pure offscreen.
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

/// A surface-shaped config so depth/pipeline creation matches the windowed path;
/// there is no real swapchain to configure offscreen.
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

    /// More adapters and devices than NVIDIA's Linux driver survived with an instance
    /// each (19), requested and dropped one after another in one process, as a long
    /// editor session's bakes are; a frame is rendered through the last. Losing the
    /// adapter part-way is the bug.
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
                // No adapter at all is the usual skip, unless one is required or named.
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
        // GL's shared EGL display is what several headless instances race on (#518).
        assert!(!HEADLESS_BACKENDS.contains(wgpu::Backends::GL));
        // …and every adapter headless renders actually run on stays reachable.
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
