//! The demo timeline: a recorded Play session opened in the Game view,
//! played, paused and scrubbed by tick, with the index's `Debug.Mark`s on the bar.

use rusty_dev::demo::{Demo, Event, Lent, Origin};

pub mod timeline;

const MAX_FRAME_TIME: f32 = 0.1;
const MAX_TICKS_PER_FRAME: u64 = 64;

impl crate::EditorUi {
    pub fn demo_open(&self) -> bool {
        self.dev.demo.is_some()
    }

    pub fn demo_size(&self) -> Option<(u32, u32)> {
        self.dev.demo.as_ref().map(|demo| demo.size)
    }
}

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
    pub seek: Option<u64>,
    pub target: Option<u64>,
    /// The timeline's close button was pressed.
    pub close: bool,
    /// How fast it plays: at the speed it was recorded.
    pub pace: Pace,
    pub size: (u32, u32),
    pub return_to: (Origin, Option<String>),
    /// The editor's own state the replay overwrote: given back on close.
    pub lent: Lent,
}

impl DemoView {
    pub fn new(demo: &Demo, return_to: (Origin, Option<String>), lent: Lent) -> Self {
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
            target: None,
            close: false,
            pace: Pace::new(demo.ticks().map(|tick| tick.dt).collect()),
            size: (1, 1),
            return_to,
            lent,
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        if !paused && self.at_end() {
            self.seek = Some(0);
        }
        self.paused = paused;
    }

    pub fn shown(&self) -> u64 {
        self.target.unwrap_or(self.current)
    }

    pub fn scrub(&mut self, tick: u64) {
        if tick > self.current {
            self.target = None;
            self.seek = Some(tick);
        } else {
            self.target = (tick != self.current).then_some(tick);
        }
    }

    pub fn release(&mut self) {
        if let Some(tick) = self.target.take() {
            self.seek = Some(tick);
        }
    }

    /// Whether the replay stands on the demo's last tick.
    pub fn at_end(&self) -> bool {
        self.current + 1 >= self.ticks
    }
}

pub struct Pace {
    dts: Vec<f32>,
    budget: f32,
}

impl Pace {
    /// The pace of ticks recorded with these `dts`, in tick order.
    pub fn new(dts: Vec<f32>) -> Self {
        Self { dts, budget: 0.0 }
    }

    pub fn spend(&mut self, current: u64, wall: f32) -> u64 {
        self.budget += wall.clamp(0.0, MAX_FRAME_TIME);
        let mut owed = 0;
        while owed < MAX_TICKS_PER_FRAME {
            let next = current + 1 + owed;
            let Some(dt) = self.dts.get(next as usize).map(|dt| dt.max(0.0)) else {
                self.budget = 0.0;
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
