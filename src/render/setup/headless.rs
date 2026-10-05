//! src/render/setup/headless.rs — Offscreen (no-window) renderer construction.
//!
//! Builds a fully functional `Renderer` with NO window surface, for the dev-layer
//! screenshot path. Reuses `Renderer::from_parts` so the offscreen renderer is
//! byte-identical to the windowed one apart from `surface == None`.
//!
//! GPU availability is NOT guaranteed in CI/containers, so adapter and device
//! acquisition return `None` (never panic) when no GPU or software adapter (e.g.
//! lavapipe) is present — the caller logs and skips gracefully.

use crate::render::Renderer;

/// The colour format the offscreen target renders into. sRGB, like the window
/// surface: the target format owns the one linear→display encode (#415), so the
/// copied-back bytes are already display-encoded PNG channel values and a headless
/// screenshot matches the window.
pub const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The backends a headless renderer may use: Vulkan, Metal and DX12 — never GL.
///
/// Every headless renderer creates its own `wgpu::Instance`, and several live at
/// once (parallel tests, a bake beside a preview). On Linux, `Backends::all()` also
/// brings up wgpu-hal's GLES/EGL backend, whose EGL display is shared process-wide:
/// dropping one instance terminates it under another thread's `make_current`, a
/// `NotInitialized` panic (#518). Every adapter a headless render runs on — a real
/// GPU, lavapipe on Linux CI, Metal on macOS, WARP on Windows — is a primary
/// backend, so leaving GL out costs no coverage. The windowed renderer keeps
/// `all()`: it owns one instance per process, and GL-only machines need it.
pub(crate) const HEADLESS_BACKENDS: wgpu::Backends = wgpu::Backends::PRIMARY;

impl Renderer {
    /// Build a headless renderer at `width` x `height`. Returns `None` (after a
    /// clear log) when no GPU/software adapter or device is available, so neither
    /// `cargo build` nor CI ever depends on a GPU at runtime.
    pub async fn new_headless(width: u32, height: u32) -> Option<Self> {
        let width = width.max(1);
        let height = height.max(1);

        // Claim a slot in the headless budget *before* allocating anything, and hold it
        // for this renderer's whole life (#366). Every headless consumer — the tests,
        // the screenshot path, the probe/reflection bakes, `Debug.Preview` — funnels
        // through here, so this is the one place the bound can be enforced for all of
        // them. Claimed before the adapter/device requests below rather than after, so
        // the count bounds live allocations; if one of those `?`s bails out, the permit
        // drops with this frame and the slot is never leaked.
        let permit = super::budget::acquire_headless();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: HEADLESS_BACKENDS,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        // No `compatible_surface` — pure offscreen. Allow a software/fallback
        // adapter (lavapipe) so headless containers can still render.
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
        {
            Ok(a) => a,
            Err(_) => instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: None,
                    force_fallback_adapter: true,
                    apply_limit_buckets: false,
                })
                .await
                .ok()?,
        };

        // Timestamps where the adapter has them: the frame stats' GPU time (#835).
        let timestamps = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: timestamps,
                required_limits: wgpu::Limits::default(),
                label: Some("Headless Screenshot Device"),
                ..Default::default()
            })
            .await
            .ok()?;

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
    use super::HEADLESS_BACKENDS;

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
