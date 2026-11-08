//! Open Image Denoise's C API, loaded at run time (#1884).
//!
//! OIDN is opened with `dlopen` (`libloading`), never linked: a `dev` build then
//! compiles and starts on a machine without it, and a bake there finds it missing
//! and filters instead, the way a `.blend` import finds Blender missing. The
//! handful of calls the lightmap filter needs are bound here, from OIDN 2's
//! `oidn.h`; nothing else in rusty names a raw OIDN handle.
//!
//! Where it looks: the [`RUSTY_OIDN`] environment variable (the library file, the
//! folder holding it, or an OIDN release's root with `lib/` inside), then the
//! loader's own search (`LD_LIBRARY_PATH`, the system folders), then Homebrew's
//! and `/usr/local`'s `lib`.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::path::{Path, PathBuf};

use libloading::Library;

/// The environment variable that names where OIDN is, when it is not installed
/// where the loader looks.
pub const RUSTY_OIDN: &str = "RUSTY_OIDN";

/// How to get OIDN, for the warning a bake without it gives.
pub const INSTALL: &str = "install Open Image Denoise 2 (macOS: `brew install \
    open-image-denoise`; Arch: `pacman -S openimagedenoise`; elsewhere the release \
    from github.com/RenderKit/oidn/releases, with RUSTY_OIDN naming its folder)";

const OIDN_DEVICE_TYPE_CPU: c_int = 1;
const OIDN_FORMAT_FLOAT3: c_int = 3;

type Device = *mut c_void;
type Filter = *mut c_void;

/// The library's file names on this platform, newest ABI first.
const NAMES: &[&str] = if cfg!(target_os = "macos") {
    &["libOpenImageDenoise.2.dylib", "libOpenImageDenoise.dylib"]
} else {
    &["libOpenImageDenoise.so.2", "libOpenImageDenoise.so"]
};

/// Folders tried after the loader's own search.
const FOLDERS: &[&str] = &["/opt/homebrew/lib", "/usr/local/lib"];

/// The calls the lightmap filter uses, and the library they live in.
pub struct Api {
    new_device: unsafe extern "C" fn(c_int) -> Device,
    commit_device: unsafe extern "C" fn(Device),
    device_error: unsafe extern "C" fn(Device, *mut *const c_char) -> c_int,
    new_filter: unsafe extern "C" fn(Device, *const c_char) -> Filter,
    /// `oidnSetSharedFilterImage`: filter, name, data, format, width, height,
    /// byte offset, pixel stride, row stride.
    set_image: unsafe extern "C" fn(
        Filter,
        *const c_char,
        *mut c_void,
        c_int,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    set_bool: unsafe extern "C" fn(Filter, *const c_char, bool),
    commit_filter: unsafe extern "C" fn(Filter),
    execute_filter: unsafe extern "C" fn(Filter),
    /// Kept open for as long as the calls above are: they point into it.
    _library: Library,
}

/// OIDN opened, with its one committed CPU device and its two `RTLightmap`
/// filters (colour, directional), each made the first time it is asked for and
/// then reused: making one loads its trained weights, about as long as cleaning a
/// small chart, and a bake cleans one or two per mesh. It lives as long as the
/// process (OIDN's devices are made once and shared), so none is ever released.
pub struct Oidn {
    api: Api,
    device: Device,
    filters: [Filter; 2],
}

// SAFETY: an OIDN device may be used from any thread, one call at a time; the one
// `Oidn` sits behind a `Mutex` (`denoise::library`), which gives exactly that.
unsafe impl Send for Oidn {}

/// Every place to try, in order: `explicit` ([`RUSTY_OIDN`]'s value) first.
pub fn candidates(explicit: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(at) = explicit {
        match at.is_dir() {
            true => {
                for dir in [at.to_path_buf(), at.join("lib")] {
                    out.extend(NAMES.iter().map(|n| dir.join(n)));
                }
            }
            false => out.push(at.to_path_buf()),
        }
    }
    out.extend(NAMES.iter().map(PathBuf::from));
    for dir in FOLDERS {
        out.extend(NAMES.iter().map(|n| Path::new(dir).join(n)));
    }
    out
}

impl Api {
    /// Open the first of `candidates` that loads and has every call.
    pub fn open(candidates: &[PathBuf]) -> Result<Self, String> {
        let mut last = "nothing to try".to_string();
        for path in candidates {
            // SAFETY: OIDN's initialisers only register its devices; a library at
            // a path the user named runs its own code, as any plugin would.
            match unsafe { Library::new(path) } {
                Ok(library) => return Self::bind(library),
                Err(e) => last = e.to_string(),
            }
        }
        Err(format!("Open Image Denoise not found ({last})"))
    }

    fn bind(library: Library) -> Result<Self, String> {
        // SAFETY: each symbol is read with its `oidn.h` signature, and the
        // pointers are kept beside the library that owns them.
        unsafe {
            macro_rules! get {
                ($name:literal) => {
                    *library
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|e| format!("{}: {e}", $name))?
                };
            }
            Ok(Self {
                new_device: get!("oidnNewDevice"),
                commit_device: get!("oidnCommitDevice"),
                device_error: get!("oidnGetDeviceError"),
                new_filter: get!("oidnNewFilter"),
                set_image: get!("oidnSetSharedFilterImage"),
                set_bool: get!("oidnSetFilterBool"),
                commit_filter: get!("oidnCommitFilter"),
                execute_filter: get!("oidnExecuteFilter"),
                _library: library,
            })
        }
    }

    /// This library with a committed CPU device.
    pub fn cpu(self) -> Result<Oidn, String> {
        // SAFETY: a device handle from `new_device` is only used through the
        // `Oidn` that owns it.
        let device = unsafe { (self.new_device)(OIDN_DEVICE_TYPE_CPU) };
        if device.is_null() {
            return Err("Open Image Denoise has no CPU device here".into());
        }
        // SAFETY: as above.
        unsafe { (self.commit_device)(device) };
        self.error(device)?;
        let filters = [std::ptr::null_mut(); 2];
        Ok(Oidn {
            api: self,
            device,
            filters,
        })
    }

    /// The device's pending error, if any, and clears it.
    fn error(&self, device: Device) -> Result<(), String> {
        let mut message: *const c_char = std::ptr::null();
        // SAFETY: OIDN writes a pointer to a string it owns, valid until the
        // device's next call; it is copied out at once.
        let code = unsafe { (self.device_error)(device, &mut message) };
        if code == 0 {
            return Ok(());
        }
        let text = match message.is_null() {
            true => String::new(),
            // SAFETY: a non-null message is a NUL-terminated C string (above).
            false => unsafe { CStr::from_ptr(message) }
                .to_string_lossy()
                .into_owned(),
        };
        Err(format!("Open Image Denoise error {code}: {text}"))
    }
}

impl Oidn {
    /// Run the `RTLightmap` filter over `color`, `size`² packed RGB floats, into
    /// `output` of the same shape. `directional` for unit-bounded direction
    /// vectors. `&mut self`: one filter at a time on the device.
    pub fn lightmap(
        &mut self,
        color: &mut [[f32; 3]],
        output: &mut [[f32; 3]],
        size: u32,
        directional: bool,
    ) -> Result<(), String> {
        let n = size as usize;
        if color.len() != n * n || output.len() != n * n {
            return Err(format!("a {n}² image of {} texels", color.len()));
        }
        let filter = self.filter(directional)?;
        // SAFETY: both images are `n`² packed FLOAT3 texels that outlive this
        // execution; the filter is re-committed with them before it runs, so it
        // never reads a previous call's images; names are C strings.
        unsafe {
            for (name, image) in [
                (c"color", color.as_mut_ptr()),
                (c"output", output.as_mut_ptr()),
            ] {
                let at = image.cast::<c_void>();
                (self.api.set_image)(filter, name.as_ptr(), at, OIDN_FORMAT_FLOAT3, n, n, 0, 0, 0);
            }
            (self.api.commit_filter)(filter);
            (self.api.execute_filter)(filter);
        }
        self.api.error(self.device)
    }

    /// The `RTLightmap` filter for colour or for `directional` vectors, made on
    /// first use.
    fn filter(&mut self, directional: bool) -> Result<Filter, String> {
        let slot = &mut self.filters[directional as usize];
        if slot.is_null() {
            // SAFETY: the device is committed and owned by `self`; names are C
            // strings.
            let filter = unsafe { (self.api.new_filter)(self.device, c"RTLightmap".as_ptr()) };
            if filter.is_null() {
                self.api.error(self.device)?;
                return Err("Open Image Denoise has no RTLightmap filter".into());
            }
            // SAFETY: as above, on the filter just made.
            unsafe { (self.api.set_bool)(filter, c"directional".as_ptr(), directional) };
            *slot = filter;
        }
        Ok(*slot)
    }
}
