//! src/audio/capture/ — record the engine's final mix to a WAV (#1579).
//!
//! **The agent cannot hear**, and the event log (`introspection`) says only which
//! sounds played and when. A capture writes the samples that reached the output,
//! after gain, pan, occlusion, group filters, the reverb bus and the speaker-mode
//! stage, so "choppy when I walk through the door" becomes a file `Sound.Level` can
//! measure, before and after a fix.
//!
//! It reads the device's final-mix tap (`device/tap.rs`). A maestro with no mixer
//! (the harness's `NullBackend`, a box with no sound card) first swaps in kira's
//! offline backend (`device/capture.rs`), keeping every live source voice; from then
//! on [`AudioMaestro::advance_output`] renders the mix in step with the engine's
//! time, so a headless capture is a pure function of the run: two runs of one
//! scenario write byte-identical files. On a real device the mix runs on the card's
//! clock and each call only drains what it played. One capture at a time.

mod wav;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::backend::PlayParams;
use super::device::capture::{Capture, RATE};
use super::device::tap::Tap;
use super::device::KiraBackend;
use super::maestro::AudioMaestro;
use wav::WavStream;

/// The longest stretch of engine time an offline mixer renders before the capture
/// drains it: one long frame (a load hitch) never overfills the tap's ring.
const MAX_RENDER_SECONDS: f64 = 1.0;

/// A running capture: where it writes, and the tap it drains.
pub struct Recorder {
    path: PathBuf,
    tap: Arc<Tap>,
    file: WavStream,
    /// What each drain moves to the file, kept so its capacity is reused.
    buffer: Vec<f32>,
    /// The first write error, reported when the capture stops.
    failed: Option<String>,
}

impl Recorder {
    /// Move everything tapped since the last drain into the file.
    fn drain(&mut self) {
        self.buffer.clear();
        self.tap.drain_into(&mut self.buffer);
        if self.failed.is_none() {
            if let Err(e) = self.file.write(&self.buffer) {
                self.failed = Some(e.to_string());
            }
        }
    }

    /// The rate the file is written at: the device's, or the offline mixer's.
    fn rate(&self) -> u32 {
        self.tap.rate().unwrap_or(RATE)
    }
}

/// A capture never stopped (the editor quit mid-capture, a run that failed after
/// `--record-audio` started) still leaves a readable WAV: the header is written
/// over its placeholder, best-effort.
impl Drop for Recorder {
    fn drop(&mut self) {
        if self.file.is_finished() {
            return;
        }
        self.tap.stop();
        self.drain();
        let rate = self.rate();
        if self.file.finish(rate).is_ok() {
            crate::core::fs::written::note_written(&self.path.to_string_lossy());
        }
    }
}

impl AudioMaestro {
    /// Start capturing the final mix into a WAV at `path`. Errors when a capture is
    /// already running or the file cannot be created.
    pub fn start_capture(&mut self, path: &Path) -> Result<(), String> {
        if let Some(running) = &self.capture {
            let at = running.path.display();
            return Err(format!("a capture into '{at}' is running; stop it first"));
        }
        // The file first: a path that cannot be written changes nothing in the run.
        let file = WavStream::create(path)
            .map_err(|e| format!("cannot write '{}': {e}", path.display()))?;
        if self.backend.tap().is_none() {
            if let Err(e) = self.go_offline() {
                discard(file, path);
                return Err(e);
            }
        }
        let Some(tap) = self.backend.tap() else {
            discard(file, path);
            return Err("the audio backend has no mix to tap".into());
        };
        tap.start();
        self.capture = Some(Recorder {
            path: path.to_path_buf(),
            tap,
            file,
            buffer: Vec::new(),
            failed: None,
        });
        Ok(())
    }

    /// `seconds` of engine time went by: an offline mixer renders them, and a
    /// running capture takes what was mixed. `GameWorld::tick` calls it once a
    /// tick, whoever drives the sim (the shell, the harness, the headless session).
    pub fn advance_output(&mut self, seconds: f64) {
        let mut left = seconds.max(0.0);
        loop {
            let step = left.min(MAX_RENDER_SECONDS);
            self.backend.advance(step);
            if let Some(capture) = self.capture.as_mut() {
                capture.drain();
            }
            left -= step;
            if left <= 0.0 {
                return;
            }
        }
    }

    /// Stop the capture and close its file, returning where it was written. Blocks
    /// the device played while the engine was not draining are lost; their count
    /// is logged as a warning, never silent.
    pub fn stop_capture(&mut self) -> Result<PathBuf, String> {
        let mut capture = self.capture.take().ok_or("no capture is running")?;
        capture.tap.stop();
        capture.drain();
        let rate = capture.rate();
        let path = capture.path.clone();
        let fail = |e: String| format!("capture into '{}' failed: {e}", path.display());
        if let Some(e) = capture.failed.take() {
            capture.file.abandon();
            return Err(fail(e));
        }
        capture.file.finish(rate).map_err(|e| fail(e.to_string()))?;
        // Noted, so a clip cache that already played this path decodes the new file.
        crate::core::fs::written::note_written(&path.to_string_lossy());
        let dropped = capture.tap.dropped_frames();
        if dropped > 0 {
            let at = path.display();
            log::warn!("[Audio] capture '{at}' lost {dropped} frames the engine drained too late");
        }
        Ok(path)
    }

    /// Where the running capture writes, if one is running.
    pub fn capture_path(&self) -> Option<&Path> {
        self.capture.as_ref().map(|c| c.path.as_path())
    }

    /// Swap in kira's offline mixer, restarting every live source voice on it (from
    /// its start: the do-nothing backend kept no playhead).
    fn go_offline(&mut self) -> Result<(), String> {
        let offline =
            KiraBackend::<Capture>::with_backend(()).ok_or("the mixer would not start")?;
        let live = std::mem::take(&mut self.entity_voices);
        self.set_backend(Box::new(offline));
        for (id, voice) in live {
            let params = PlayParams {
                clip: voice.source.clip.clone(),
                looping: voice.source.looping,
                mix: self.output(voice.mix),
                group: voice.group,
            };
            if self.backend.play(voice.voice, &params) {
                self.entity_voices.insert(id, voice);
            } else {
                let clip = &voice.source.clip;
                log::warn!("[Audio] entity {id}'s clip '{clip}' did not restart for the capture");
            }
        }
        Ok(())
    }
}

/// Close and delete a capture file a refused start created.
fn discard(mut file: WavStream, path: &Path) {
    file.abandon();
    drop(file);
    let _ = std::fs::remove_file(path);
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod capture_tests;
