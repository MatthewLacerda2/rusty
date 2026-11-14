//! Where depth priming (#1499) runs. Unity URP's *Auto* mode primes only where it
//! is recommended: never on its mobile targets (Android, iOS, tvOS, embedded Linux),
//! whose tile-based GPUs remove hidden opaque fragments in hardware, and never on
//! WebGL or WebGPU, which render it wrong on Apple GPUs (`UniversalRendererRenderGraph
//! .cs`, `IsDepthPrimingEnabledRenderGraph`). rusty keys the same rule by backend:
//! Metal means an Apple GPU, which is tile-based, so priming buys nothing there, and
//! on it the prepass depth and the forward pass's are not bit-identical (CI's macOS
//! leg, 2026-10-28). URP still primes standalone macOS; rusty doesn't, since a pixel
//! the prepass and the forward pass disagree on would draw the wrong surface.

/// Whether a renderer on `backend` primes its forward pass with the SSAO prepass
/// depth. The front-to-back sort runs everywhere either way.
pub(crate) fn recommended(backend: wgpu::Backend) -> bool {
    backend != wgpu::Backend::Metal
}
