//! Memory as the platform reports it: the process's peak resident memory, for
//! `make bench-scenes`' `rss_peak_mb` (each scene runs in a process of its own, so
//! the peak is that scene's) and a bake's report (#2312), and the machine's free
//! and total memory, which a lightmap bake's estimate is held to before it starts
//! (`bake::lightmap::fits`).
//!
//! Linux reads `VmHWM` from `/proc/self/status` and `MemAvailable` / `MemTotal`
//! from `/proc/meminfo`; macOS reads the peak physical footprint
//! `task_info(TASK_VM_INFO)` reports (`ledger_phys_footprint_peak`, what Activity
//! Monitor's Memory column counts), the free, inactive, purgeable and speculative
//! pages `host_statistics64` counts (what a process can have without swapping),
//! and `hw.memsize`.

/// The process's peak resident memory in bytes; `None` where it cannot be read.
pub fn rss_peak_bytes() -> Option<u64> {
    peak()
}

/// Have the allocator hand every large block it frees back to the system at once
/// (#2312), for a bake: glibc's malloc otherwise raises its mmap threshold each
/// time a large block is freed, so a bake's per-chart buffers (megabytes each)
/// come from its heap, and the holes they leave between the lightmaps kept stay
/// resident; a scene of 4 times the meshes peaked about twice its extra maps
/// higher. Pinning the threshold at its default (128 KiB) turns the raising off.
/// Process-wide and lasting, which costs a page fault per large block; idempotent.
/// macOS's allocator already returns large blocks, and others are left alone.
pub fn return_large_frees() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        /// `M_MMAP_THRESHOLD` in glibc's `malloc.h`.
        const M_MMAP_THRESHOLD: std::ffi::c_int = -3;
        unsafe extern "C" {
            fn mallopt(param: std::ffi::c_int, value: std::ffi::c_int) -> std::ffi::c_int;
        }
        // SAFETY: `mallopt` only sets a malloc tunable; any value is accepted or
        // refused (returning 0), never undefined.
        unsafe { mallopt(M_MMAP_THRESHOLD, 128 << 10) };
    }
}

/// Hand the blocks the allocator holds free back to the system (#2312), after a
/// bake's chart and each page it encodes: macOS's allocator keeps freed multi-MB
/// blocks in its regions. On
/// glibc, [`return_large_frees`] does it as each block is freed; elsewhere, nothing.
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

/// What the allocator holds from the system and what of it is in use, in bytes
/// (#2312): a test that finds a peak too high says which it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heap {
    pub in_use: u64,
    pub held: u64,
}

/// The allocator's [`Heap`] now: glibc's `mallinfo2` over every arena, macOS's
/// `malloc_zone_statistics` over every zone; `None` elsewhere.
pub fn heap() -> Option<Heap> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        /// glibc's `struct mallinfo2`: ten `size_t`.
        #[repr(C)]
        struct Info([usize; 10]);
        unsafe extern "C" {
            fn mallinfo2() -> Info;
        }
        // SAFETY: `mallinfo2` takes nothing and returns its struct by value.
        let Info(f) = unsafe { mallinfo2() };
        // arena, ordblks, smblks, hblks, hblkhd, usmblks, fsmblks, uordblks, …
        let (arena, mapped, used) = (f[0] as u64, f[4] as u64, f[7] as u64);
        Some(Heap {
            in_use: used + mapped,
            held: arena + mapped,
        })
    }
    #[cfg(target_os = "macos")]
    {
        /// libmalloc's `malloc_statistics_t`.
        #[repr(C)]
        #[derive(Default)]
        #[allow(dead_code, reason = "filled by the C call; two fields are not read")]
        struct Stats {
            blocks_in_use: u32,
            size_in_use: usize,
            max_size_in_use: usize,
            size_allocated: usize,
        }
        unsafe extern "C" {
            fn malloc_zone_statistics(zone: *mut std::ffi::c_void, stats: *mut Stats);
        }
        let mut stats = Stats::default();
        // SAFETY: a null zone sums every zone into `stats`, a `malloc_statistics_t`.
        unsafe { malloc_zone_statistics(std::ptr::null_mut(), &mut stats) };
        Some(Heap {
            in_use: stats.size_in_use as u64,
            held: stats.size_allocated as u64,
        })
    }
    #[cfg(not(any(all(target_os = "linux", target_env = "gnu"), target_os = "macos")))]
    {
        None
    }
}

/// The machine's memory, in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Machine {
    /// What a process could take now without the machine swapping.
    pub available: u64,
    /// All of it.
    pub total: u64,
}

/// The machine's memory now; `None` where it cannot be read.
pub fn machine() -> Option<Machine> {
    machine_now()
}

#[cfg(target_os = "linux")]
fn peak() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    kb_line(&status, "VmHWM:")
}

#[cfg(target_os = "linux")]
fn machine_now() -> Option<Machine> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    Some(Machine {
        available: kb_line(&info, "MemAvailable:")?,
        total: kb_line(&info, "MemTotal:")?,
    })
}

/// The `<key>   123456 kB` line of a `/proc` text (`VmHWM:` in `status`,
/// `MemAvailable:` in `meminfo`), in bytes.
#[cfg(any(target_os = "linux", test))]
fn kb_line(text: &str, key: &str) -> Option<u64> {
    let line = text.lines().find(|l| l.starts_with(key))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(target_os = "macos")]
fn peak() -> Option<u64> {
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
    let info = unsafe { info.assume_init() };
    u64::try_from(info.ledger_phys_footprint_peak).ok()
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
    let pages = [
        stats.free_count,
        stats.inactive_count,
        stats.purgeable_count,
        stats.speculative_count,
    ];
    let available = pages.iter().map(|&n| u64::from(n) * page).sum();
    Some(Machine { available, total })
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn peak() -> Option<u64> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn machine_now() -> Option<Machine> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proc_line_reads_in_bytes() {
        let status = "Name:\tplay\nVmPeak:\t  900 kB\nVmHWM:\t  2048 kB\nVmRSS:\t 1024 kB\n";
        assert_eq!(kb_line(status, "VmHWM:"), Some(2048 * 1024));
        assert_eq!(kb_line("Name:\tplay\n", "VmHWM:"), None);
        assert_eq!(kb_line("VmHWM:\tlots kB\n", "VmHWM:"), None);
        let info = "MemTotal:       16000000 kB\nMemFree: 1 kB\nMemAvailable:    9000000 kB\n";
        assert_eq!(kb_line(info, "MemAvailable:"), Some(9_000_000 * 1024));
    }

    #[test]
    fn the_heap_in_use_is_within_what_it_holds() {
        let held = vec![1u8; 4 << 20];
        if let Some(h) = heap() {
            assert!(h.in_use >= held.len() as u64 && h.in_use <= h.held, "{h:?}");
        }
    }

    #[test]
    fn the_machine_is_read_on_linux_and_macos_and_has_some_free() {
        let machine = machine();
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            let m = machine.expect("read");
            assert!(
                m.total > 0 && m.available > 0 && m.available <= m.total,
                "{m:?}"
            );
        }
    }

    #[test]
    fn the_peak_is_read_and_positive_on_linux_and_macos() {
        let peak = rss_peak_bytes();
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            assert!(peak.is_some_and(|b| b > 0), "{peak:?}");
        }
    }
}
