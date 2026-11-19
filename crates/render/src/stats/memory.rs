//! How much GPU memory the renderer's device holds (`make bench-scenes`' `gpu_mb`).
//! Each backend answers from the one source it has: Vulkan and DX12 from wgpu's
//! allocator report, its `total_reserved_bytes` (the memory blocks the allocator
//! took from the device, live allocations and their free space alike: what the
//! device has given the process), Metal from the device's `currentAllocatedSize`
//! (every byte the device allocated, the driver's own included). The two are
//! close but not equal: Metal's also counts what the driver keeps for itself.
//! Without either (GL) there is no answer.

/// A device's GPU memory, and which source gave it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuMemory {
    pub bytes: u64,
    /// `"allocator report, reserved blocks"` or `"Metal currentAllocatedSize"`.
    pub source: &'static str,
    /// What of `bytes` live allocations use (the allocator report's
    /// `total_allocated_bytes`), where the source tells it apart: not on Metal,
    /// whose figure is allocated bytes already.
    pub allocated: Option<u64>,
}

/// The GPU memory `device` holds once it has caught up with the work queued, so
/// what was dropped before the call (a bench's clock probe) is freed and not
/// counted; `None` when its backend reports none.
pub fn gpu_memory(device: &wgpu::Device) -> Option<GpuMemory> {
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    if let Some(report) = device.generate_allocator_report() {
        return Some(GpuMemory {
            bytes: report.total_reserved_bytes,
            source: "allocator report, reserved blocks",
            allocated: Some(report.total_allocated_bytes),
        });
    }
    metal_allocated(device)
}

#[cfg(target_os = "macos")]
fn metal_allocated(device: &wgpu::Device) -> Option<GpuMemory> {
    use objc2_metal::MTLDevice;
    // SAFETY: the hal device is only read (one property getter) while the guard
    // `as_hal` returns is alive, and no wgpu call runs on this thread meanwhile.
    let hal = unsafe { device.as_hal::<wgpu::hal::api::Metal>() }?;
    let bytes = hal.raw_device().currentAllocatedSize() as u64;
    Some(GpuMemory {
        bytes,
        source: "Metal currentAllocatedSize",
        allocated: None,
    })
}

#[cfg(not(target_os = "macos"))]
fn metal_allocated(_device: &wgpu::Device) -> Option<GpuMemory> {
    None
}
