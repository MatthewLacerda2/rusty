//! src/audio/device/capture.rs — a kira backend that renders into memory (tests).
//!
//! kira's own `MockBackend` renders but keeps its output to itself; this one hands
//! the frames back, so a test hears the whole kira graph (voices, group tracks,
//! filters, the output stage) with no sound card.

use kira::backend::{Backend, Renderer};

/// The sample rate the captured graph runs at.
pub const RATE: u32 = 48_000;

/// A kira backend whose output a test pulls with [`Capture::render`].
pub struct Capture {
    renderer: Option<Renderer>,
}

impl Backend for Capture {
    type Settings = ();
    type Error = ();

    fn setup(_settings: (), _buffer: usize) -> Result<(Self, u32), ()> {
        Ok((Self { renderer: None }, RATE))
    }

    fn start(&mut self, renderer: Renderer) -> Result<(), ()> {
        self.renderer = Some(renderer);
        Ok(())
    }
}

impl Capture {
    /// Render the next `frames` stereo frames, applying every command sent since the
    /// last render first (as the device's next callback would).
    pub fn render(&mut self, frames: usize) -> Vec<(f32, f32)> {
        let renderer = self.renderer.as_mut().expect("the manager started");
        renderer.on_start_processing();
        let mut out = vec![0.0; frames * 2];
        renderer.process(&mut out, 2);
        out.chunks(2).map(|f| (f[0], f[1])).collect()
    }
}
