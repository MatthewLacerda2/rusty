//! How much GPU memory the renderer's device holds (`make bench-scenes`' `gpu_mb`).
//! Each backend answers from the one source it has: Vulkan and DX12 from wgpu's
//! allocator report (the bytes live allocations take), Metal from the device's
//! `currentAllocatedSize` (every byte the device allocated, the driver's own
//! included). Without either (GL) there is no answer.

/// A device's GPU memory, and which source gave it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuMemory {
    pub bytes: u64,
    /// `"allocator report"` or `"Metal currentAllocatedSize"`.
    pub source: &'static str,
}

/// The GPU memory `device` holds now; `None` when its backend reports none.
pub fn gpu_memory(device: &wgpu::Device) -> Option<GpuMemory> {
    if let Some(report) = device.generate_allocator_report() {
        return Some(GpuMemory {
            bytes: report.total_allocated_bytes,
            source: "allocator report",
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
    })
}

#[cfg(not(target_os = "macos"))]
fn metal_allocated(_device: &wgpu::Device) -> Option<GpuMemory> {
    None
}
