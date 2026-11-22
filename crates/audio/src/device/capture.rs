use kira::backend::{Backend, DefaultBackend, Renderer};

/// The sample rate the captured graph runs at.
pub const RATE: u32 = 48_000;

/// Not kira's `MockBackend`, which keeps what it renders: this hands the frames back,
/// pulled, never pushed by a device callback.
pub struct Capture {
    renderer: Option<Renderer>,
    /// Seconds asked for but not yet rendered: the fraction of a frame each
    /// [`Clocked::advance`] leaves over, so 60 ticks of 1/60 s render 48 000 frames.
    owed: f64,
}

impl Backend for Capture {
    type Settings = ();
    type Error = ();

    fn setup(_settings: (), _buffer: usize) -> Result<(Self, u32), ()> {
        Ok((
            Self {
                renderer: None,
                owed: 0.0,
            },
            RATE,
        ))
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
        let mut out = vec![0.0; frames * 2];
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.on_start_processing();
            renderer.process(&mut out, 2);
        }
        out.chunks(2).map(|f| (f[0], f[1])).collect()
    }
}

pub trait Clocked {
    /// `seconds` of the engine's time went by.
    fn advance(&mut self, seconds: f64);
}

impl Clocked for Capture {
    fn advance(&mut self, seconds: f64) {
        self.owed += seconds.max(0.0);
        let frames = (self.owed * f64::from(RATE)).floor();
        self.owed -= frames / f64::from(RATE);
        self.render(frames as usize);
    }
}

/// The sound card's callback pulls samples on its own clock.
impl Clocked for DefaultBackend {
    fn advance(&mut self, _seconds: f64) {}
}
