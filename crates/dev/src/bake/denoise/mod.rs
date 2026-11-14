//! crates/dev/src/bake/denoise/ — the lightmap bake's trained denoiser (#1884): Intel
//! Open Image Denoise's `RTLightmap` filter behind the sim's [`LightmapDenoiser`].
//!
//! The bake asks for one through [`lent`] when its settings pick
//! [`Denoiser::Oidn`]: the library is opened once per process ([`ffi`], at run
//! time, so no build needs it; a failed open is tried again at the next bake, so
//! installing OIDN needs no restart), and one [`Oidn`] per bake holds its devices
//! and counts its charts and their time, for the bake log's denoise line. Where
//! OIDN is missing the bake is lent nothing and filters, after one warning that
//! says what to install.
//!
//! Each worker cleans its own charts on a device of its own, one thread each, so
//! the denoise runs beside the trace with no lock between workers. On the
//! Dust2-sized level (`scale_tests`, #1884's review) OIDN added about a sixth to
//! the trace's time either way on 8 threads, but one shared device of every core
//! spent 70 s behind one lock against a 134 s trace: on a machine that traces
//! twice as fast, that lock would set the bake's pace.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use glam::Vec3;

use rusty_scene::lighting::settings::{BakeSettings, Denoiser};
use rusty_lighting::lightmap::LightmapDenoiser;

pub mod ffi;

pub use ffi::{INSTALL, RUSTY_OIDN};

/// The process's OIDN library, opened the first time a bake asks; why not, when it
/// failed. Only a success is kept: a failure is tried again on the next call.
fn library() -> Result<&'static ffi::Api, String> {
    static OPENED: Mutex<Option<&'static ffi::Api>> = Mutex::new(None);
    let mut opened = OPENED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(api) = *opened {
        return Ok(api);
    }
    let explicit = std::env::var_os(RUSTY_OIDN).map(PathBuf::from);
    let api: &'static ffi::Api = Box::leak(Box::new(ffi::Api::open(&ffi::candidates(
        explicit.as_deref(),
    ))?));
    *opened = Some(api);
    Ok(api)
}

/// Whether this machine can run OIDN (it opens and makes a device), and why not.
pub fn available() -> Result<(), String> {
    library()?.device(Some(1)).map(drop)
}

/// One bake's use of OIDN: its devices, made as workers first need them, and the
/// charts it cleaned and the time they took.
pub struct Oidn {
    devices: Vec<Mutex<Option<ffi::Device>>>,
    next: AtomicUsize,
    charts: AtomicU64,
    nanos: AtomicU64,
    warned: AtomicBool,
}

impl Default for Oidn {
    /// One single-threaded device per core: one per bake worker.
    fn default() -> Self {
        Self::with_devices(std::thread::available_parallelism().map_or(1, |n| n.get()))
    }
}

impl Oidn {
    /// `count` devices, each on one thread; one device alone runs on every core
    /// (OIDN's own threads), the bake's workers taking turns at it.
    pub fn with_devices(count: usize) -> Self {
        Self {
            devices: (0..count.max(1)).map(|_| Mutex::new(None)).collect(),
            next: AtomicUsize::new(0),
            charts: AtomicU64::new(0),
            nanos: AtomicU64::new(0),
            warned: AtomicBool::new(false),
        }
    }

    /// The charts cleaned so far and OIDN's time on them, summed over the devices
    /// (with several, more than the wall time it added).
    pub fn spent(&self) -> (u64, Duration) {
        let nanos = self.nanos.load(Ordering::Relaxed);
        (
            self.charts.load(Ordering::Relaxed),
            Duration::from_nanos(nanos),
        )
    }

    /// A device slot no other worker holds, or, when all are busy, the next one's
    /// turn.
    fn slot(&self) -> MutexGuard<'_, Option<ffi::Device>> {
        let start = self.next.fetch_add(1, Ordering::Relaxed);
        let n = self.devices.len();
        let free = (0..n).find_map(|k| self.devices[(start + k) % n].try_lock().ok());
        free.unwrap_or_else(|| {
            let busy = &self.devices[start % n];
            busy.lock().unwrap_or_else(PoisonError::into_inner)
        })
    }

    /// Clean one image on a device of this bake's, made on first use.
    fn run(
        &self,
        color: &mut [[f32; 3]],
        output: &mut [[f32; 3]],
        size: u32,
        directional: bool,
    ) -> Result<(), String> {
        let api = library()?;
        let mut slot = self.slot();
        if slot.is_none() {
            let threads = (self.devices.len() > 1).then_some(1);
            *slot = Some(api.device(threads)?);
        }
        let device = slot.as_mut().expect("made above");
        let from = Instant::now();
        let ran = device.lightmap(color, output, size, directional);
        self.nanos
            .fetch_add(from.elapsed().as_nanos() as u64, Ordering::Relaxed);
        ran
    }
}

impl LightmapDenoiser for Oidn {
    fn denoise(&self, image: &mut [Vec3], size: u32, directional: bool) -> Result<(), String> {
        let mut color: Vec<[f32; 3]> = image.iter().map(|v| v.to_array()).collect();
        let mut output = vec![[0.0; 3]; color.len()];
        let ran = self.run(&mut color, &mut output, size, directional);
        if let Err(why) = &ran {
            if !self.warned.swap(true, Ordering::Relaxed) {
                log::warn!("[LightmapBake] OIDN failed on a lightmap, filtering it: {why}");
            }
            return ran;
        }
        // A chart's colour is cleaned once; a directional bake's second call, its
        // directions, is the same chart.
        if !directional {
            self.charts.fetch_add(1, Ordering::Relaxed);
        }
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
            log::warn!("[LightmapBake] {}", missing(&why, settings.samples));
            None
        }
    }
}

/// The warning a bake without OIDN gives: why, what to install, and that the
/// Gaussian at the lower default samples looks noisier.
pub fn missing(why: &str, samples: u32) -> String {
    format!(
        "{why}: filtering instead. To denoise, {INSTALL}. \
         The filter at {samples} samples looks noisier than OIDN; raise Samples \
         (128 or more) or pick the Filter denoiser to silence this."
    )
}

/// The environment variable CI sets so a missing OIDN fails the canary test
/// instead of letting every OIDN test skip (as `RUSTY_REQUIRE_GPU` does for the GPU).
pub const REQUIRE_OIDN_ENV: &str = "RUSTY_REQUIRE_OIDN";

#[cfg(test)]
mod bake_tests;
#[cfg(test)]
mod quality;
#[cfg(test)]
mod quality_tests;
#[cfg(test)]
mod scale_tests;
#[cfg(test)]
mod tests;
