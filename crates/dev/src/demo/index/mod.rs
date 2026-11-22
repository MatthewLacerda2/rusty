use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::format::plain_json;

mod find;
pub use find::{find_in, EventKind, Query};
use rusty_app::GameWorld;
use rusty_scene::{Scene, SceneId};
use rusty_script::scripting::LogLevel;

/// Ticks between two camera samples: four a second at 60 Hz.
pub const CAMERA_EVERY: u64 = 15;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IndexLine {
    pub at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Mark {
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
    },
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
    Scene {
        path: String,
    },
    Play {
        state: String,
    },
}

pub struct Indexer {
    names: BTreeMap<u32, String>,
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

    pub fn end(&mut self, world: &GameWorld, ticks: u64) -> IndexLine {
        self.gather(world);
        self.events.push(play("stopped"));
        world.console().borrow_mut().feed_demo(false);
        IndexLine {
            // Stamped at the last tick: a search finds only ticks a seek can reach.
            at: ticks.saturating_sub(1),
            camera: None,
            events: std::mem::take(&mut self.events),
        }
    }

    fn gather(&mut self, world: &GameWorld) {
        // First, so the engine's own log lines land in the feed of the tick that raised them.
        rusty_logging::forward(world.console());
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

    fn rebase(&mut self, scene: &Scene) {
        let ecs = &scene.world;
        self.ids = ecs.ids().to_vec();
        self.names = self
            .ids
            .iter()
            .map(|&id| (id, name_of(scene, id)))
            .collect();
    }

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
