mod wav;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::backend::PlayParams;
use super::device::capture::{Capture, RATE};
use super::device::tap::Tap;
use super::device::KiraBackend;
use super::maestro::AudioMaestro;
use wav::WavStream;

/// Engine seconds an offline render covers before a drain: a load hitch never overfills
/// the tap's ring.
const MAX_RENDER_SECONDS: f64 = 1.0;

pub struct Recorder {
    path: PathBuf,
    tap: Arc<Tap>,
    file: WavStream,
    buffer: Vec<f32>,
    failed: Option<String>,
    ticks: u64,
    paused_ticks: u64,
}

impl Recorder {
    fn drain(&mut self) {
        self.buffer.clear();
        self.tap.drain_into(&mut self.buffer);
        if self.failed.is_none() {
            if let Err(e) = self.file.write(&self.buffer) {
                self.failed = Some(e.to_string());
            }
        }
    }

    fn rate(&self) -> u32 {
        self.tap.rate().unwrap_or(RATE)
    }

    fn close(&mut self) -> Result<(), String> {
        self.tap.stop();
        self.drain();
        let finished = self.file.finish(self.rate());
        if finished.is_ok() {
            rusty_core::fs::written::note_written(&self.path.to_string_lossy());
        }
        self.report();
        let fail = |e: String| format!("capture into '{}' failed: {e}", self.path.display());
        match (self.failed.take(), finished) {
            (Some(e), _) => Err(fail(e)),
            (None, Err(e)) => Err(fail(e.to_string())),
            (None, Ok(())) => Ok(()),
        }
    }

    fn report(&self) {
        let at = self.path.display();
        let dropped = self.tap.dropped_frames();
        if dropped > 0 {
            log::warn!(
                "[Audio] capture '{at}' lost {dropped} frames the engine drained too late; \
                 they are silence in the file"
            );
        }
        if self.ticks > 0 && self.paused_ticks == self.ticks {
            log::warn!(
                "[Audio] capture '{at}': every tick of this capture ran paused: time-scaled \
                 sources were held; capture a run with `play --record-audio` or \
                 `Harness.RecordAudio`"
            );
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if self.file.is_finished() {
            return;
        }
        if let Err(e) = self.close() {
            log::warn!("[Audio] {e}");
        }
    }
}

impl AudioMaestro {
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
            ticks: 0,
            paused_ticks: 0,
        });
        Ok(())
    }

    /// `seconds` of engine time went by, `paused` or not: an offline mixer renders
    /// them, and a running capture takes what was mixed. `GameWorld::tick` calls it
    /// once a tick, whoever drives the sim (the shell, the harness, the headless
    /// session); a paused frame that ran no tick calls it with no time.
    pub fn advance_output(&mut self, seconds: f64, paused: bool) {
        if let Some(capture) = self.capture.as_mut().filter(|_| seconds > 0.0) {
            capture.ticks += 1;
            capture.paused_ticks += u64::from(paused);
        }
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

    pub fn stop_capture(&mut self) -> Result<PathBuf, String> {
        let mut capture = self.capture.take().ok_or("no capture is running")?;
        capture.close()?;
        Ok(capture.path.clone())
    }

    pub fn capture_path(&self) -> Option<&Path> {
        self.capture.as_ref().map(|c| c.path.as_path())
    }

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

fn discard(mut file: WavStream, path: &Path) {
    file.abandon();
    drop(file);
    let _ = std::fs::remove_file(path);
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod capture_tests;

#[cfg(test)]
#[path = "doppler_tests.rs"]
mod doppler_tests;

#[cfg(test)]
#[path = "loud_tests.rs"]
mod loud_tests;

#[cfg(test)]
#[path = "loud_hall_tests.rs"]
mod loud_hall_tests;

#[cfg(test)]
#[path = "master_hall_tests.rs"]
mod master_hall_tests;
