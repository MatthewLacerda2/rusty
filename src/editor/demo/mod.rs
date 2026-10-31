//! The demo timeline (#1598): a recorded Play session opened in the Game view,
//! played, paused and scrubbed by tick, with the index's `Debug.Mark`s on the bar.
//! Unity's closest analog is the Profiler/Timeline scrubbing a recorded capture.
//!
//! This is the editor's side, state and widget only: the shell frontend
//! (`shell::editor::demo`) owns the replay and drives the world from it. Playback
//! is read-only, as the replay is the demo's input and nothing typed in the editor
//! feeds it.

use crate::dev::demo::{Demo, Event, Lent};
use crate::scene::SceneSnapshot;

pub mod timeline;

/// The most wall time one frame spends on ticks, so a stall is not a fast-forward.
const MAX_FRAME_TIME: f32 = 0.1;
/// The most ticks one frame runs, for a run of ticks recorded with no `dt`.
const MAX_TICKS_PER_FRAME: u64 = 64;

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
    /// How fast it plays: at the speed it was recorded.
    pub pace: Pace,
    /// The Game view size the replay lays out on, `(width, height)` in pixels: the
    /// view draws it letterboxed in the panel.
    pub size: (u32, u32),
    /// The edit scene the editor had before the demo took the world: put back on close.
    pub return_to: (SceneSnapshot, Option<String>),
    /// The editor's own state the replay overwrote: given back on close.
    pub lent: Lent,
}

impl DemoView {
    /// The view of `demo`, paused on its first tick; `return_to` and `lent` are
    /// what to put back.
    pub fn new(demo: &Demo, return_to: (SceneSnapshot, Option<String>), lent: Lent) -> Self {
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
            pace: Pace::new(demo.ticks().map(|tick| tick.dt).collect()),
            size: (1, 1),
            return_to,
            lent,
        }
    }

    /// Pause, or play: play on the last tick starts over, as a media player does.
    /// The timeline's button and `Editor.PauseDemo` both come here.
    pub fn set_paused(&mut self, paused: bool) {
        if !paused && self.at_end() {
            self.seek = Some(0);
        }
        self.paused = paused;
    }

    /// Whether the replay stands on the demo's last tick.
    pub fn at_end(&self) -> bool {
        self.current + 1 >= self.ticks
    }
}

/// Playback's clock: wall time spent against each tick's recorded `dt`, so a demo
/// plays at the speed it was recorded whatever the display's rate.
pub struct Pace {
    dts: Vec<f32>,
    /// Wall-clock time not yet spent on ticks.
    budget: f32,
}

impl Pace {
    /// The pace of ticks recorded with these `dts`, in tick order.
    pub fn new(dts: Vec<f32>) -> Self {
        Self { dts, budget: 0.0 }
    }

    /// Spend `wall` seconds of a frame against the `dt`s of the ticks after
    /// `current`: how many to run now. What is left carries to the next frame.
    pub fn spend(&mut self, current: u64, wall: f32) -> u64 {
        self.budget += wall.clamp(0.0, MAX_FRAME_TIME);
        let mut owed = 0;
        while owed < MAX_TICKS_PER_FRAME {
            let next = current + 1 + owed;
            // `max` maps a NaN to 0: a broken `dt` costs nothing, never stalls.
            let Some(dt) = self.dts.get(next as usize).map(|dt| dt.max(0.0)) else {
                self.budget = 0.0; // the end holds still
                break;
            };
            if self.budget < dt {
                break;
            }
            self.budget -= dt;
            owed += 1;
        }
        owed
    }
}
