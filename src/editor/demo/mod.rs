//! The demo timeline (#1598): a recorded Play session opened in the Game view,
//! played, paused and scrubbed by tick, with the index's `Debug.Mark`s on the bar.
//! Unity's closest analog is the Profiler/Timeline scrubbing a recorded capture.
//!
//! This is the editor's side, state and widget only: the shell frontend
//! (`shell::editor::demo`) owns the replay and drives the world from it. Playback
//! is read-only, as the replay is the demo's input and nothing typed in the editor
//! feeds it.

use crate::dev::demo::{Demo, Event};
use crate::scene::SceneSnapshot;

pub mod timeline;

/// A demo open in the Game view.
pub struct DemoView {
    pub path: String,
    /// How many ticks the demo holds.
    pub ticks: u64,
    /// The last tick the replay ran.
    pub current: u64,
    pub paused: bool,
    /// The index's `Debug.Mark`s: `(tick, label)`, in tick order.
    pub marks: Vec<(u64, String)>,
    /// A tick the timeline asked to replay to, taken by the frontend next frame.
    pub seek: Option<u64>,
    /// The timeline's close button was pressed.
    pub close: bool,
    /// Wall-clock time not yet spent on ticks: playback runs at 60 ticks a second
    /// whatever the display's rate.
    pub budget: f32,
    /// The edit scene the editor had before the demo took the world: put back on close.
    pub return_to: (SceneSnapshot, Option<String>),
}

impl DemoView {
    /// The view of `demo`, paused on its first tick; `return_to` is what to put back.
    pub fn new(demo: &Demo, return_to: (SceneSnapshot, Option<String>)) -> Self {
        let marks = demo
            .index()
            .flat_map(|line| {
                let at = line.at;
                line.events
                    .into_iter()
                    .filter_map(move |event| match event {
                        Event::Mark { label, .. } => Some((at, label)),
                        _ => None,
                    })
            })
            .collect();
        Self {
            path: demo.path().to_string_lossy().into_owned(),
            ticks: demo.ticks,
            current: 0,
            paused: true,
            marks,
            seek: None,
            close: false,
            budget: 0.0,
            return_to,
        }
    }

    /// Whether the replay stands on the demo's last tick.
    pub fn at_end(&self) -> bool {
        self.current + 1 >= self.ticks
    }
}
