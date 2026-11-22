//! The process's and the machine's memory as Linux and macOS report it; what each figure
//! counts: `docs/testing.md` (`rss_peak_mb`) and `docs/api/Lighting.md`.

pub fn rss_peak_bytes() -> Option<u64> {
    peak()
}

pub fn rss_now_bytes() -> Option<u64> {
    now()
}

pub fn return_large_frees() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // `M_MMAP_THRESHOLD` in glibc's `malloc.h`; pinned at its default, 128 KiB, so glibc
        // stops raising it as large blocks are freed.
        const M_MMAP_THRESHOLD: std::ffi::c_int = -3;
        unsafe extern "C" {
            fn mallopt(param: std::ffi::c_int, value: std::ffi::c_int) -> std::ffi::c_int;
        }
        // SAFETY: `mallopt` only sets a malloc tunable; any value is accepted or
        // refused (returning 0), never undefined.
        unsafe { mallopt(M_MMAP_THRESHOLD, 128 << 10) };
    }
}

pub fn relieve() {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
        }
        // SAFETY: a null zone asks every zone, and a goal of 0 as much as each can
        // give; it only returns free pages to the system.
        unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
    }
}

#[cfg(test)]
mod decoded_gpu_tests;
mod heap;
#[cfg(test)]
mod lightmaps_gpu_tests;
#[cfg(test)]
mod textures_gpu_tests;
pub use heap::{heap, Heap};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Machine {
    pub available: u64,
    pub total: u64,
}

pub fn machine() -> Option<Machine> {
    machine_now()
}

#[cfg(target_os = "linux")]
fn peak() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    kb_line(&status, "VmHWM:")
}

#[cfg(target_os = "linux")]
fn now() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    kb_line(&status, "VmRSS:")
}

#[cfg(target_os = "linux")]
fn machine_now() -> Option<Machine> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let host = Machine {
        available: kb_line(&info, "MemAvailable:")?,
        total: kb_line(&info, "MemTotal:")?,
    };
    let own = std::fs::read_to_string("/proc/self/cgroup").unwrap_or_default();
    let capped = cgroup::own_path(&own).and_then(|p| cgroup::bound(cgroup::ROOT.as_ref(), p));
    Some(capped.map_or(host, |cap| host.within(cap)))
}

impl Machine {
    pub fn within(self, cap: Machine) -> Machine {
        Machine {
            available: self.available.min(cap.available),
            total: self.total.min(cap.total),
        }
    }
}

#[cfg(any(target_os = "linux", test))]
fn kb_line(text: &str, key: &str) -> Option<u64> {
    let line = text.lines().find(|l| l.starts_with(key))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(target_os = "macos")]
fn peak() -> Option<u64> {
    u64::try_from(vm_info()?.ledger_phys_footprint_peak).ok()
}

#[cfg(target_os = "macos")]
fn now() -> Option<u64> {
    u64::try_from(vm_info()?.phys_footprint).ok()
}

#[cfg(target_os = "macos")]
fn vm_info() -> Option<mach2::task_info::task_vm_info> {
    use mach2::kern_return::KERN_SUCCESS;
    use mach2::message::mach_msg_type_number_t;
    use mach2::task::task_info;
    use mach2::task_info::{task_vm_info, TASK_VM_INFO};
    use mach2::traps::mach_task_self;

    let mut info = std::mem::MaybeUninit::<task_vm_info>::zeroed();
    let mut count = (std::mem::size_of::<task_vm_info>() / std::mem::size_of::<u32>())
        as mach_msg_type_number_t;
    // SAFETY: `info` is a zeroed `task_vm_info` and `count` its size in 32-bit
    // words, as `task_info` asks; it writes at most `count` words into it.
    let status = unsafe {
        task_info(
            mach_task_self(),
            TASK_VM_INFO,
            info.as_mut_ptr().cast(),
            &mut count,
        )
    };
    if status != KERN_SUCCESS {
        return None;
    }
    // SAFETY: `task_info` succeeded, so it filled the struct (zeroed beyond `count`).
    Some(unsafe { info.assume_init() })
}

#[cfg(target_os = "macos")]
fn machine_now() -> Option<Machine> {
    use mach2::kern_return::{kern_return_t, KERN_SUCCESS};
    use mach2::mach_init::mach_host_self;
    use mach2::message::mach_msg_type_number_t;
    use mach2::vm_statistics::vm_statistics64;

    /// `HOST_VM_INFO64` in `mach/host_info.h`.
    const HOST_VM_INFO64: i32 = 4;
    unsafe extern "C" {
        fn host_statistics64(
            host: u32,
            flavor: i32,
            info: *mut i32,
            count: *mut mach_msg_type_number_t,
        ) -> kern_return_t;
        fn sysctlbyname(
            name: *const std::ffi::c_char,
            old: *mut std::ffi::c_void,
            old_len: *mut usize,
            new: *mut std::ffi::c_void,
            new_len: usize,
        ) -> i32;
    }
    let mut total = 0u64;
    let mut len = std::mem::size_of::<u64>();
    // SAFETY: `hw.memsize` is a 64-bit integer, written into `total`, whose size
    // `len` gives; nothing is set.
    let read = unsafe {
        sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&raw mut total).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if read != 0 {
        return None;
    }
    let mut stats = vm_statistics64::default();
    let mut count = (std::mem::size_of::<vm_statistics64>() / std::mem::size_of::<i32>())
        as mach_msg_type_number_t;
    // SAFETY: `stats` is a `vm_statistics64`, `count` its size in 32-bit words,
    // as `HOST_VM_INFO64` asks; the host port is this task's own.
    let status = unsafe {
        host_statistics64(
            mach_host_self(),
            HOST_VM_INFO64,
            (&raw mut stats).cast(),
            &mut count,
        )
    };
    if status != KERN_SUCCESS {
        return None;
    }
    // SAFETY: `vm_page_size` is set by the system before `main`, never written after.
    let page = unsafe { mach2::vm_page_size::vm_page_size } as u64;
    // Free counts the speculative pages already and purgeable ones sit on these queues, so
    // adding either counts a page twice. `vm_stat` prints free less speculative, hence
    // `memory_floor.py`'s three: keep the two sums alike.
    let available = (u64::from(stats.free_count) + u64::from(stats.inactive_count)) * page;
    Some(Machine { available, total })
}

#[cfg(any(target_os = "linux", test))]
mod cgroup;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn peak() -> Option<u64> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn now() -> Option<u64> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn machine_now() -> Option<Machine> {
    None
}

#[cfg(test)]
mod tests;
