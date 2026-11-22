//! A test hook: the allocator's own count, in bytes, which a memory test's failure
//! message prints to say whether a peak was held or in use.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heap {
    pub in_use: u64,
    pub held: u64,
}

pub fn heap() -> Option<Heap> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        #[repr(C)]
        struct Info([usize; 10]);
        unsafe extern "C" {
            fn mallinfo2() -> Info;
        }
        // SAFETY: `mallinfo2` takes nothing and returns its struct by value.
        let Info(f) = unsafe { mallinfo2() };
        // `mallinfo2`'s fields in order: arena, ordblks, smblks, hblks, hblkhd, usmblks,
        // fsmblks, uordblks, …
        let (arena, mapped, used) = (f[0] as u64, f[4] as u64, f[7] as u64);
        Some(Heap {
            in_use: used + mapped,
            held: arena + mapped,
        })
    }
    #[cfg(target_os = "macos")]
    {
        /// libmalloc's `malloc_statistics_t`, field for field.
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
