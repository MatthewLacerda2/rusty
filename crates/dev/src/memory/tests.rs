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

#[test]
fn the_resident_memory_now_is_read_and_within_the_peak() {
    let now = rss_now_bytes();
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        let now = now.expect("read");
        assert!(
            now > 0 && rss_peak_bytes().is_some_and(|p| now <= p),
            "{now}"
        );
    }
}
