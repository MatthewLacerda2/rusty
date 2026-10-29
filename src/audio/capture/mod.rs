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

/// A running capture: where it writes, and the tap it drains.
pub struct Recorder {
    path: PathBuf,
    tap: Arc<Tap>,
    file: WavStream,
    /// The first write error, reported when the capture stops.
    failed: Option<String>,
}

impl Recorder {
    /// Move everything tapped since the last drain into the file.
    fn drain(&mut self) {
        let samples = self.tap.take();
        if self.failed.is_none() {
            if let Err(e) = self.file.write(&samples) {
                self.failed = Some(e.to_string());
            }
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
        if self.backend.tap().is_none() {
            self.go_offline()?;
        }
        let tap = self
            .backend
            .tap()
            .ok_or("the audio backend has no mix to tap")?;
        let file = WavStream::create(path)
            .map_err(|e| format!("cannot write '{}': {e}", path.display()))?;
        tap.start();
        self.capture = Some(Recorder {
            path: path.to_path_buf(),
            tap,
            file,
            failed: None,
        });
        Ok(())
    }

    /// `seconds` of engine time went by: an offline mixer renders them, and a
    /// running capture takes what was mixed. Called once a frame by the shell and
    /// once a tick by the harness.
    pub fn advance_output(&mut self, seconds: f64) {
        self.backend.advance(seconds);
        if let Some(capture) = self.capture.as_mut() {
            capture.drain();
        }
    }

    /// Stop the capture and close its file, returning where it was written.
    pub fn stop_capture(&mut self) -> Result<PathBuf, String> {
        let mut capture = self.capture.take().ok_or("no capture is running")?;
        capture.tap.stop();
        capture.drain();
        let rate = capture.tap.rate().unwrap_or(RATE);
        let path = capture.path;
        let fail = |e: String| format!("capture into '{}' failed: {e}", path.display());
        if let Some(e) = capture.failed {
            return Err(fail(e));
        }
        capture.file.finish(rate).map_err(|e| fail(e.to_string()))?;
        // Noted, so a clip cache that already played this path decodes the new file.
        crate::core::fs::written::note_written(&path.to_string_lossy());
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
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod capture_tests;
