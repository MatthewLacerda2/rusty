//! src/dev/bake/denoise/ — the lightmap bake's trained denoiser (#1884): Intel
//! Open Image Denoise's `RTLightmap` filter behind the sim's [`LightmapDenoiser`].
//!
//! The bake asks for one through [`lent`] when its settings pick
//! [`Denoiser::Oidn`]: the library is opened once per process ([`ffi`], at run
//! time, so no build needs it), and one [`Oidn`] per bake counts the time its
//! charts took, for the bake log's denoise line. Where OIDN is missing the bake is
//! lent nothing and filters, after one warning that says what to install. OIDN
//! runs its own threads, so the bake's workers take turns at it, one chart at a
//! time.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use glam::Vec3;

use crate::scene::lighting::lightmap::{BakeSettings, Denoiser, LightmapDenoiser};

pub mod ffi;

pub use ffi::{INSTALL, RUSTY_OIDN};

/// The process's OIDN, opened the first time a bake asks, behind the lock every
/// filter run takes; why not, when it failed.
fn library() -> Result<&'static Mutex<ffi::Oidn>, &'static str> {
    static OPENED: OnceLock<Result<Mutex<ffi::Oidn>, String>> = OnceLock::new();
    let opened = OPENED.get_or_init(|| {
        let explicit = std::env::var_os(RUSTY_OIDN).map(PathBuf::from);
        let api = ffi::Api::open(&ffi::candidates(explicit.as_deref()))?;
        Ok(Mutex::new(api.cpu()?))
    });
    opened.as_ref().map_err(String::as_str)
}

/// Whether this machine can run OIDN, and why not.
pub fn available() -> Result<(), String> {
    library().map(|_| ()).map_err(str::to_string)
}

/// One bake's use of OIDN: the charts it cleaned and the time they took.
#[derive(Default)]
pub struct Oidn {
    charts: AtomicU64,
    nanos: AtomicU64,
    warned: AtomicBool,
}

impl Oidn {
    /// The charts cleaned so far and their time, summed over the workers.
    pub fn spent(&self) -> (u64, Duration) {
        let nanos = self.nanos.load(Ordering::Relaxed);
        (
            self.charts.load(Ordering::Relaxed),
            Duration::from_nanos(nanos),
        )
    }
}

impl LightmapDenoiser for Oidn {
    fn denoise(&self, image: &mut [Vec3], size: u32, directional: bool) -> Result<(), String> {
        let library = library()?;
        let mut color: Vec<[f32; 3]> = image.iter().map(|v| v.to_array()).collect();
        let mut output = vec![[0.0; 3]; color.len()];
        let ran = {
            let mut oidn = library.lock().unwrap_or_else(PoisonError::into_inner);
            let from = Instant::now();
            let ran = oidn.lightmap(&mut color, &mut output, size, directional);
            self.nanos
                .fetch_add(from.elapsed().as_nanos() as u64, Ordering::Relaxed);
            ran
        };
        if let Err(why) = &ran {
            if !self.warned.swap(true, Ordering::Relaxed) {
                log::warn!("[LightmapBake] OIDN failed on a lightmap, filtering it: {why}");
            }
            return ran;
        }
        self.charts.fetch_add(1, Ordering::Relaxed);
        for (v, o) in image.iter_mut().zip(output) {
            *v = Vec3::from_array(o);
        }
        Ok(())
    }
}

/// The denoiser a bake with `settings` is lent: an [`Oidn`] when it picks
/// [`Denoiser::Oidn`] and the library opens; else `None`, the bake filters, and a
/// missing library is warned about once for this bake.
pub fn lent(settings: &BakeSettings) -> Option<Arc<Oidn>> {
    if settings.denoiser != Denoiser::Oidn {
        return None;
    }
    match library() {
        Ok(_) => Some(Arc::new(Oidn::default())),
        Err(why) => {
            log::warn!("{}", missing(why, settings.samples));
            None
        }
    }
}

/// The warning a bake without OIDN gives: why, what to install, and that the
/// Gaussian at the lower default samples looks noisier.
pub fn missing(why: &str, samples: u32) -> String {
    format!(
        "[LightmapBake] {why}: filtering instead. To denoise, {INSTALL}. \
         The filter at {samples} samples looks noisier than OIDN; raise Samples \
         (128 or more) or pick the Filter denoiser to silence this."
    )
}
