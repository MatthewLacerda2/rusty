//! Metal means an Apple GPU (tile-based), where the prepass depth and the forward pass's are not bit-identical, so priming is off there; URP's *Auto* mode skips mobile and WebGL, and still primes macOS.

/// The front-to-back sort runs everywhere either way.
pub(crate) fn recommended(backend: wgpu::Backend) -> bool {
    backend != wgpu::Backend::Metal
}
