//! The adapter-free half of the priming gate (#1499): which backends prime.

use super::priming::recommended;

#[test]
fn metal_never_primes() {
    assert!(!recommended(wgpu::Backend::Metal));
}

#[test]
fn the_desktop_backends_prime() {
    for backend in [
        wgpu::Backend::Vulkan,
        wgpu::Backend::Dx12,
        wgpu::Backend::Gl,
    ] {
        assert!(recommended(backend), "{backend:?}");
    }
}
