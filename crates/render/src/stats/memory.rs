#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuMemory {
    pub bytes: u64,
    pub source: &'static str,
    pub allocated: Option<u64>,
}

pub fn gpu_memory(device: &wgpu::Device) -> Option<GpuMemory> {
    // Caught up first, so what was dropped before the call is freed, not counted.
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
    // SAFETY: the hal device is only read (one property getter) while `as_hal`'s guard lives.
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
