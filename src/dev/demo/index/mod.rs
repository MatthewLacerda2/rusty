//! A demo's index (#1597): what happened when, written beside the tick lines as it
//! happens, so a search finds a moment without replaying anything.
//!
//! ```text
//! {"at":0,"camera":[0.0,5.0,-10.0],"events":[{"play":{"state":"entered"}},{"spawned":{"id":42,"name":"Bullet"}}]}
//! {"at":15,"camera":[0.1,5.0,-9.8]}
//! {"at":73,"events":[{"mark":{"label":"took_damage","value":12}},{"log":{"text":"ouch","level":"info"}}]}
//! ```
//!
//! An index line follows its tick's line (`at` is the tick, as a tick line's `t`).
//! The active camera's position is sampled every [`CAMERA_EVERY`] ticks (a
//! non-finite one is skipped: JSON has no NaN); the events are the ones the engine
//! knows (an entity spawned or destroyed, a scene loaded, Play entered, paused,
//! resumed or stopped, every console line, each repeat of one included) and the
//! game facts a script marks with `Debug.Mark(label [, value])`. The entities that
//! come and go with a scene load are not listed: the load is the event. The last
//! index line, at the demo's last tick, carries what came after it and Play
//! stopping.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::format::plain_json;

mod find;
use crate::app::GameWorld;
use crate::scene::{Scene, SceneId};
use crate::scripting::LogLevel;
pub use find::{find_in, EventKind, Query};

/// Ticks between two camera samples: four a second at 60 Hz.
pub const CAMERA_EVERY: u64 = 15;

/// One index line: a camera sample, the events of a tick, or both.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IndexLine {
    pub at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
}

/// Something that happened at a tick.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    /// A `Debug.Mark(label [, value])`.
    Mark {
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
    },
    /// A console line: `level` is `info`, `warning` or `error`.
    Log {
        text: String,
        level: String,
    },
    Spawned {
        id: u32,
        name: String,
    },
    Destroyed {
        id: u32,
        name: String,
    },
    /// A scene was loaded (`Scene.Load`); `path` is its file.
    Scene {
        path: String,
    },
    /// Play was `entered`, `paused`, `resumed` or `stopped`.
    Play {
        state: String,
    },
}

/// Gathers a recording's index, tick by tick.
pub struct Indexer {
    /// The live entities' names, by id.
    names: BTreeMap<u32, String>,
    /// The scene's ids as the last tick left them: unchanged, nothing came or went.
    ids: Vec<u32>,
    scene: SceneId,
    paused: bool,
    events: Vec<Event>,
}

fn play(state: &str) -> Event {
    Event::Play {
        state: state.into(),
    }
}

fn level_name(level: &LogLevel) -> &'static str {
    match level {
        LogLevel::Info => "info",
        LogLevel::Warning => "warning",
        LogLevel::Error => "error",
    }
}

impl Indexer {
    /// Right before Play is entered: the world as it stands is the baseline, and
    /// the console feeds the index from now on.
    pub fn start(world: &GameWorld) -> Self {
        world.console().borrow_mut().feed_demo(true);
        let scene = world.scene().borrow();
        let mut indexer = Self {
            names: BTreeMap::new(),
            ids: Vec::new(),
            scene: scene.id(),
            paused: world.time().borrow().paused,
            events: vec![play("entered")],
        };
        indexer.rebase(&scene);
        indexer
    }

    /// The index line of tick `t`, just run; `None` when it has nothing to say.
    pub fn line(&mut self, world: &GameWorld, t: u64) -> Option<IndexLine> {
        self.gather(world);
        let position = world.camera().borrow().position;
        let camera =
            (t.is_multiple_of(CAMERA_EVERY) && position.is_finite()).then(|| position.to_array());
        let line = IndexLine {
            at: t,
            camera,
            events: std::mem::take(&mut self.events),
        };
        (line.camera.is_some() || !line.events.is_empty()).then_some(line)
    }

    /// The last line, right before Stop, of a demo of `ticks` ticks: what came
    /// after the last tick, and Play stopping, stamped at that tick (a search
    /// finds only ticks a seek can reach). The console stops feeding the index.
    pub fn end(&mut self, world: &GameWorld, ticks: u64) -> IndexLine {
        self.gather(world);
        self.events.push(play("stopped"));
        world.console().borrow_mut().feed_demo(false);
        IndexLine {
            at: ticks.saturating_sub(1),
            camera: None,
            events: std::mem::take(&mut self.events),
        }
    }

    fn gather(&mut self, world: &GameWorld) {
        // The engine's own lines join the console at the tick that raised them.
        crate::logging::forward(world.console());
        let feed = world.console().borrow_mut().take_feed();
        for mark in feed.marks {
            let value = mark.value.as_ref().map(plain_json);
            let label = mark.label;
            self.events.push(Event::Mark { label, value });
        }
        for (text, level) in feed.lines {
            let level = level_name(&level).into();
            self.events.push(Event::Log { text, level });
        }
        let paused = world.time().borrow().paused;
        if paused != self.paused {
            self.paused = paused;
            self.events
                .push(play(if paused { "paused" } else { "resumed" }));
        }
        let scene = world.scene().borrow();
        if scene.id() != self.scene {
            self.scene = scene.id();
            let path = world.script_manager().scene_path_cell().borrow().clone();
            self.events.push(Event::Scene {
                path: path.unwrap_or_default(),
            });
            self.rebase(&scene);
        } else {
            self.entities(&scene);
        }
    }

    /// Take `scene`'s entities as they are, with no event.
    fn rebase(&mut self, scene: &Scene) {
        let ecs = &scene.world;
        self.ids = ecs.ids().to_vec();
        self.names = self
            .ids
            .iter()
            .map(|&id| (id, name_of(scene, id)))
            .collect();
    }

    /// The entities spawned and destroyed since the last look.
    fn entities(&mut self, scene: &Scene) {
        let ids = scene.world.ids();
        if ids == self.ids.as_slice() {
            return;
        }
        let now: BTreeSet<u32> = ids.iter().copied().collect();
        let gone: Vec<u32> = self
            .names
            .keys()
            .filter(|id| !now.contains(id))
            .copied()
            .collect();
        for id in gone {
            let name = self.names.remove(&id).unwrap_or_default();
            self.events.push(Event::Destroyed { id, name });
        }
        for &id in ids {
            if let std::collections::btree_map::Entry::Vacant(slot) = self.names.entry(id) {
                let name = slot.insert(name_of(scene, id)).clone();
                self.events.push(Event::Spawned { id, name });
            }
        }
        self.ids = ids.to_vec();
    }
}

fn name_of(scene: &Scene, id: u32) -> String {
    scene
        .world
        .name(id)
        .map(|n| n.to_string())
        .unwrap_or_default()
}
