//! The process's peak resident memory, for `make bench-scenes`' `rss_peak_mb`.
//! Each scene runs in a process of its own, so the peak is that scene's.
//! Linux reads `VmHWM` from `/proc/self/status`; macOS reads the peak physical
//! footprint `task_info(TASK_VM_INFO)` reports (`ledger_phys_footprint_peak`,
//! what Activity Monitor's Memory column counts).

/// The process's peak resident memory in bytes; `None` where it cannot be read.
pub fn rss_peak_bytes() -> Option<u64> {
    peak()
}

#[cfg(target_os = "linux")]
fn peak() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    vm_hwm(&status)
}

/// `VmHWM:   123456 kB` in a `/proc/<pid>/status` text, in bytes.
#[cfg_attr(
    not(target_os = "linux"),
    expect(dead_code, reason = "read on Linux only")
)]
fn vm_hwm(status: &str) -> Option<u64> {
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
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

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn peak() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_hwm_reads_the_peak_line_in_bytes() {
        let status = "Name:\tplay\nVmPeak:\t  900 kB\nVmHWM:\t  2048 kB\nVmRSS:\t 1024 kB\n";
        assert_eq!(vm_hwm(status), Some(2048 * 1024));
        assert_eq!(vm_hwm("Name:\tplay\n"), None);
        assert_eq!(vm_hwm("VmHWM:\tlots kB\n"), None);
    }

    #[test]
    fn the_peak_is_read_and_positive_on_linux_and_macos() {
        let peak = rss_peak_bytes();
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            assert!(peak.is_some_and(|b| b > 0), "{peak:?}");
        }
    }
}
