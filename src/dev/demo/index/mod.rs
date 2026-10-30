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
//! The active camera's position is sampled every [`CAMERA_EVERY`] ticks; the
//! events are the ones the engine knows (an entity spawned or destroyed, a scene
//! loaded, Play entered, paused, resumed or stopped, every console line) and the
//! game facts a script marks with `Debug.Mark(label [, value])`. The entities that
//! come and go with a scene load are not listed: the load is the event. The last
//! index line, at the demo's end tick, carries what came after the last tick.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::format::plain_json;

mod find;
use crate::app::GameWorld;
use crate::scene::{Scene, SceneId};
use crate::scripting::LogLevel;
pub use find::{EventKind, Query};

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
    /// The console's count of lines when last read.
    logged: u64,
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
    /// `Debug.Mark`s are kept from now on.
    pub fn start(world: &GameWorld) -> Self {
        let mut console = world.console().borrow_mut();
        console.keep_marks(true);
        let scene = world.scene().borrow();
        let mut indexer = Self {
            names: BTreeMap::new(),
            ids: Vec::new(),
            scene: scene.id(),
            paused: world.time().borrow().paused,
            logged: console.logged(),
            events: vec![play("entered")],
        };
        indexer.rebase(&scene);
        indexer
    }

    /// The index line of tick `t`, just run; `None` when it has nothing to say.
    pub fn line(&mut self, world: &GameWorld, t: u64) -> Option<IndexLine> {
        self.gather(world);
        let camera = t.is_multiple_of(CAMERA_EVERY).then(|| {
            let position = world.camera().borrow().position;
            position.to_array()
        });
        let line = IndexLine {
            at: t,
            camera,
            events: std::mem::take(&mut self.events),
        };
        (line.camera.is_some() || !line.events.is_empty()).then_some(line)
    }

    /// The last line, at `end`, right before Stop: what came after the last tick,
    /// and Play stopping. Marks are no longer kept.
    pub fn end(&mut self, world: &GameWorld, end: u64) -> IndexLine {
        self.gather(world);
        self.events.push(play("stopped"));
        world.console().borrow_mut().keep_marks(false);
        IndexLine {
            at: end,
            camera: None,
            events: std::mem::take(&mut self.events),
        }
    }

    fn gather(&mut self, world: &GameWorld) {
        let mut console = world.console().borrow_mut();
        for mark in console.take_marks() {
            let value = mark.value.as_ref().map(plain_json);
            let label = mark.label;
            self.events.push(Event::Mark { label, value });
        }
        for (text, level) in console.since(self.logged) {
            let (text, level) = (text.clone(), level_name(level).into());
            self.events.push(Event::Log { text, level });
        }
        self.logged = console.logged();
        drop(console);
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
