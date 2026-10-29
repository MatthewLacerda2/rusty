//! The replay: a [`TickTap`] that feeds a demo back into a world, tick by tick,
//! and checks the world against the demo's digests (#1581).
//!
//! Before each tick it repeats the host acts recorded for it (a console line, a
//! scenario's API call) and sets the Game view size; right before input is
//! published it writes the recorded [`InputFrame`](crate::core::input::InputFrame)
//! over whatever live input there was; after the tick it compares the digest,
//! when one was stored. The first tick whose digest differs is the divergence, and
//! a tainted tick is warned about from there on: a replay never drifts in silence.

use mlua::Lua;

use super::calls;
use super::read::{Demo, DemoTick};
use crate::app::{GameWorld, TapPoint, TickTap};
use crate::core::settings::video::{VideoSettings, VIDEO_NAMESPACE};
use crate::core::settings::{saved_quality, GraphicsSettings};
use crate::scripting::HostCall;

/// The first digest that differs from the recording: the state went astray after
/// `matched` (the last digest that agreed, if any) and by `tick`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    pub tick: u64,
    pub matched: Option<u64>,
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for Divergence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "diverged by tick {}", self.tick)?;
        match self.matched {
            Some(t) => write!(f, " (the state still matched at tick {t})"),
            None => write!(f, " (no earlier digest matched)"),
        }
    }
}

/// How a replay went, so far.
#[derive(Clone, Debug, Default)]
pub struct Verdict {
    /// Ticks replayed.
    pub ticks: u64,
    /// Ticks the demo holds.
    pub of: u64,
    /// Digests compared.
    pub checked: u64,
    /// The last tick whose digest agreed.
    pub matched: Option<u64>,
    pub diverged: Option<Divergence>,
    /// `(tick, what)` for every act the demo could not carry.
    pub taints: Vec<(u64, String)>,
}

/// Feeds a demo into the world it is installed on; see [`prepare`].
pub struct Replayer {
    ticks: std::vec::IntoIter<DemoTick>,
    tick: Option<DemoTick>,
    lua: Lua,
    verdict: Verdict,
}

impl Replayer {
    pub fn verdict(&self) -> &Verdict {
        &self.verdict
    }

    /// Whether every tick of the demo was replayed.
    pub fn finished(&self) -> bool {
        self.verdict.ticks >= self.verdict.of
    }

    fn act(&mut self, world: &mut GameWorld, t: u64, act: HostCall) {
        let console = world.console().clone();
        let failed = match act {
            HostCall::Eval(line) => world.script_manager().eval(&line).err(),
            HostCall::Call { function, args } => {
                let cells = world.script_manager().api_cells();
                calls::invoke(&self.lua, &cells, &function, &args).err()
            }
            HostCall::Taint(what) => {
                let line = format!(
                    "[Demo] tainted from tick {t}: {what}; the replay may differ from here"
                );
                console.borrow_mut().warn(line);
                self.verdict.taints.push((t, what));
                None
            }
        };
        if let Some(err) = failed {
            console
                .borrow_mut()
                .error(format!("[Demo] tick {t}: {err}"));
        }
    }
}

/// Set `world` up as `demo` started — its seed, save data, the options and quality
/// the player had saved, the Game view size — and install the replay. Call before
/// the world's first tick; the project settings go in when the harness is built.
pub fn prepare(world: &mut GameWorld, demo: Demo) -> Result<(), String> {
    let header = &demo.header;
    let running = crate::core::project::engine_commit();
    if header.engine_commit != running {
        let line = format!(
            "[Demo] recorded on engine {}, replayed on {running}: it may differ",
            header.engine_commit
        );
        world.console().borrow_mut().warn(line);
    }
    world.resources.script_manager.set_play_seed(header.seed);
    let storage_cell = world.resources.storage.clone();
    let mut storage = storage_cell.borrow_mut();
    storage.replace(header.storage.clone());
    let video = storage.get_namespace(VIDEO_NAMESPACE);
    *world.script_manager().video_cell().borrow_mut() = video
        .map_or_else(VideoSettings::default, |blob| {
            VideoSettings::from_json(&blob)
        });
    let mut app = world.resources.application.borrow_mut();
    app.restore_quality(saved_quality(&storage));
    let graphics = GraphicsSettings::load(&storage, app.graphics_defaults(), app.quality_presets());
    *world.script_manager().graphics_cell().borrow_mut() = graphics;
    drop((app, storage));
    if let Some((width, height)) = header.size {
        world
            .resources
            .screen
            .borrow_mut()
            .set_game_view(width, height);
    }
    let lua = crate::scripting::sandboxed_vm()?;
    let of = demo.ticks;
    let ticks: Vec<DemoTick> = demo.ticks().collect();
    world.set_tap(Some(Box::new(Replayer {
        ticks: ticks.into_iter(),
        tick: None,
        lua,
        verdict: Verdict {
            of,
            ..Verdict::default()
        },
    })));
    Ok(())
}

impl TickTap for Replayer {
    fn at(&mut self, point: TapPoint, world: &mut GameWorld) {
        match point {
            TapPoint::BeforeTick => {
                self.tick = self.ticks.next();
                let Some(tick) = &mut self.tick else {
                    return;
                };
                let (t, acts, size) = (tick.t, std::mem::take(&mut tick.host), tick.size);
                for act in acts {
                    self.act(world, t, act);
                }
                if let Some((width, height)) = size {
                    world
                        .resources
                        .screen
                        .borrow_mut()
                        .set_game_view(width, height);
                }
            }
            TapPoint::BeforeInput => {
                if let Some(tick) = &self.tick {
                    world.input().borrow_mut().set_frame(&tick.input);
                }
            }
            TapPoint::AfterTick => {
                let Some(tick) = self.tick.take() else {
                    return;
                };
                self.verdict.ticks = tick.t + 1;
                let Some(expected) = tick.digest else {
                    return;
                };
                self.verdict.checked += 1;
                let actual = super::digest::of(world);
                if actual == expected {
                    if self.verdict.diverged.is_none() {
                        self.verdict.matched = Some(tick.t);
                    }
                } else if self.verdict.diverged.is_none() {
                    let divergence = Divergence {
                        tick: tick.t,
                        matched: self.verdict.matched,
                        expected,
                        actual,
                    };
                    world
                        .console()
                        .borrow_mut()
                        .error(format!("[Demo] {divergence}"));
                    self.verdict.diverged = Some(divergence);
                }
            }
            TapPoint::Transition(_) => {}
        }
    }
}
