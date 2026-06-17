//! src/render/setup_headless.rs — Offscreen (no-window) renderer construction.
//!
//! Builds a fully functional `Renderer` with NO window surface, for the dev-layer
//! screenshot path. Reuses `Renderer::from_parts` so the offscreen renderer is
//! byte-identical to the windowed one apart from `surface == None`.
//!
//! GPU availability is NOT guaranteed in CI/containers, so adapter and device
//! acquisition return `None` (never panic) when no GPU or software adapter (e.g.
//! lavapipe) is present — the caller logs and skips gracefully.

use super::Renderer;

/// The colour format the offscreen target renders into. Non-sRGB so the copied-back
/// bytes map 1:1 to PNG channel values without a gamma surprise.
pub const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl Renderer {
    /// Build a headless renderer at `width` x `height`. Returns `None` (after a
    /// clear log) when no GPU/software adapter or device is available, so neither
    /// `cargo build` nor CI ever depends on a GPU at runtime.
    pub async fn new_headless(width: u32, height: u32) -> Option<Self> {
        let width = width.max(1);
        let height = height.max(1);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // No `compatible_surface` — pure offscreen. Allow a software/fallback
        // adapter (lavapipe) so headless containers can still render.
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
        {
            Some(a) => a,
            None => {
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface: None,
                        force_fallback_adapter: true,
                    })
                    .await?
            }
        };

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: Some("Headless Screenshot Device"),
                },
                None,
            )
            .await
            .ok()?;

        let size = winit::dpi::PhysicalSize::new(width, height);
        // No real swapchain offscreen: only the always-available `Fifo` mode.
        Some(Self::from_parts(
            device,
            queue,
            None,
            offscreen_config(width, height),
            size,
            vec![wgpu::PresentMode::Fifo],
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
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}
