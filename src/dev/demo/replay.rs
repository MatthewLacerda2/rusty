//! The replay: a [`TickTap`] that feeds a demo back into a world, tick by tick,
//! and checks the world against the demo's digests (#1581).
//!
//! Right before Play is entered it repeats the acts made before it (a scenario's
//! set-up); right before each tick's input is published it repeats the host acts
//! recorded for it (a console line, a scenario's API call), sets the Game view
//! size and writes the recorded [`InputFrame`](crate::core::input::InputFrame)
//! over whatever live input there was, and the tick runs on the recorded `dt`;
//! after the tick it compares the digest, when one was stored, and after the
//! last the digest Play ended on. The first tick whose digest differs is the divergence, and
//! a tainted tick is warned about from there on: a replay never drifts in silence.

use mlua::Lua;

use super::calls;
use super::format::{host_call, End, Header};
use super::read::{Demo, DemoTick, DemoTicks};
use super::sources::Sources;
use crate::app::{GameWorld, PlayTransition, TapPoint, TickTap};
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
    /// Whether a scenario on top steps the replay: it may end it before the
    /// demo's end on purpose.
    pub scenario: bool,
}

/// Feeds a demo into the world it is installed on; see [`prepare`].
pub struct Replayer {
    ticks: DemoTicks,
    tick: Option<DemoTick>,
    before_play: Vec<HostCall>,
    end: End,
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

    /// A scenario on top steps this replay, so stopping short is its choice.
    pub fn driven_by_scenario(&mut self) {
        self.verdict.scenario = true;
    }

    fn act(&mut self, world: &mut GameWorld, t: u64, act: HostCall) {
        let console = world.console().clone();
        let failed = match act {
            HostCall::Eval(line) => world.script_manager().eval(&line).err(),
            HostCall::Call { function, args } if function.starts_with("Harness.") => {
                let verb = &function["Harness.".len()..];
                calls::invoke_harness(&self.lua, world.scene(), verb, &args).err()
            }
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

    fn acts(&mut self, world: &mut GameWorld, t: u64, acts: Vec<HostCall>) {
        for act in acts {
            self.act(world, t, act);
        }
    }
}

/// Warn when the demo was recorded on another engine build, or on a project whose
/// scene file or scripts changed since: it replays anyway.
fn warn_changes(world: &GameWorld, header: &Header) {
    let running = crate::core::project::engine_commit();
    let mut console = world.console().borrow_mut();
    if header.engine_commit != running {
        let line = format!(
            "[Demo] recorded on engine {}, replayed on {running}: it may differ",
            header.engine_commit
        );
        console.warn(line);
    }
    if let Some(what) = header.sources.changed(&Sources::of(world)) {
        console.warn(format!(
            "[Demo] the project's {what} changed since it was recorded: it may differ"
        ));
    }
}

/// Set `world` up as `demo` started — its seed, save data, the options and quality
/// the player had saved, the time scale and pause, the Game view size — and install the
/// replay. Call before the world's first tick; the project settings go in when
/// the harness is built.
pub fn prepare(world: &mut GameWorld, demo: Demo) -> Result<(), String> {
    let header = &demo.header;
    warn_changes(world, header);
    world.resources.script_manager.set_play_seed(header.seed);
    // An `f32` widened to `f64` narrows back exactly.
    let mut time = world.resources.time.borrow_mut();
    time.time_scale = header.time_scale.0 as f32;
    (time.paused, time.pending_steps) = (header.paused, header.pending_steps);
    drop(time);
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
    let before_play = header.before_play.iter().filter_map(host_call).collect();
    let verdict = Verdict {
        of: demo.ticks,
        ..Verdict::default()
    };
    world.set_tap(Some(Box::new(Replayer {
        ticks: demo.ticks(),
        tick: None,
        before_play,
        end: demo.end,
        lua,
        verdict,
    })));
    Ok(())
}

impl Replayer {
    /// Right before Play is entered: the acts made before it. A line run in edit
    /// mode needs the edit-mode runtime the recording host had (the editor's, the
    /// session's), which a headless replay brings up for it.
    fn before_play(&mut self, world: &mut GameWorld) {
        let acts = std::mem::take(&mut self.before_play);
        if acts.iter().any(|act| matches!(act, HostCall::Eval(_))) {
            let init = world.init_edit_runtime();
            world.report_lua_init(init);
        }
        self.acts(world, 0, acts);
    }

    /// Before input is published: the tick's host acts again, and its Game view
    /// size. After the transition, so the acts made between entering Play and
    /// the first tick land after entering it, however it was entered.
    fn before_input(&mut self, world: &mut GameWorld) {
        let Some(tick) = &mut self.tick else {
            return;
        };
        let (t, acts, size) = (tick.t, std::mem::take(&mut tick.host), tick.size);
        self.acts(world, t, acts);
        if let Some((width, height)) = size {
            world
                .resources
                .screen
                .borrow_mut()
                .set_game_view(width, height);
        }
        if let Some(tick) = &self.tick {
            world.input().borrow_mut().set_frame(&tick.input);
        }
    }

    /// After the tick: its digest compared, when the demo stored one; after the
    /// last, the acts made before Stop and the digest Play ended on.
    fn after_tick(&mut self, world: &mut GameWorld) {
        let Some(tick) = self.tick.take() else {
            return;
        };
        self.verdict.ticks = tick.t + 1;
        if let Some(expected) = tick.digest {
            self.compare(world, tick.t, expected);
        }
        if self.finished() {
            let end = std::mem::take(&mut self.end);
            let acts = end.host.iter().filter_map(host_call).collect();
            self.acts(world, end.end, acts);
            if let Some(expected) = end.digest {
                self.compare(world, end.end, expected);
            }
        }
    }

    /// Compare the world's digest with `expected`, stored at tick `t`.
    fn compare(&mut self, world: &GameWorld, t: u64, expected: String) {
        self.verdict.checked += 1;
        if self.verdict.diverged.is_some() {
            return;
        }
        let actual = super::digest::of(world);
        if actual == expected {
            self.verdict.matched = Some(t);
            return;
        }
        let divergence = Divergence {
            tick: t,
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

impl TickTap for Replayer {
    fn at(&mut self, point: TapPoint, world: &mut GameWorld) {
        match point {
            TapPoint::Before(PlayTransition::Entered) => self.before_play(world),
            TapPoint::BeforeTick => self.tick = self.ticks.next(),
            TapPoint::BeforeInput => self.before_input(world),
            TapPoint::AfterTick => self.after_tick(world),
            TapPoint::Before(_) | TapPoint::Transition(_) | TapPoint::AfterPoll => {}
        }
    }

    fn dt(&mut self, dt: f32) -> f32 {
        self.tick.as_ref().map_or(dt, |tick| tick.dt)
    }
}
