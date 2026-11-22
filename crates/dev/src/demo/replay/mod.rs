use mlua::Lua;

use super::format::{host_call, End};
use super::read::{Demo, DemoTick, DemoTicks};
use super::{calls, script_ref};
mod cut;
mod lend;
mod origin;
mod seek;
mod verdict;
pub use cut::{cut, full_path, scenario as cut_scenario};
use lend::Host;
pub use lend::{lend, Lent};
pub use origin::Origin;
use rusty_app::{GameWorld, PlayTransition, TapPoint, TickTap};
use rusty_core::settings::video::{VideoSettings, VIDEO_NAMESPACE};
use rusty_core::settings::{saved_quality, GraphicsSettings};
use rusty_script::scripting::HostCall;
use seek::Start;
pub use seek::{seek, NO_DEMO};
pub use verdict::{Divergence, Verdict};

pub struct Replayer {
    ticks: DemoTicks,
    tick: Option<DemoTick>,
    before_play: Vec<HostCall>,
    end: End,
    lua: Lua,
    verdict: Verdict,
    start: Start,
    past_end: bool,
}

impl Replayer {
    pub fn verdict(&self) -> &Verdict {
        &self.verdict
    }

    pub fn finished(&self) -> bool {
        self.verdict.ticks >= self.verdict.of
    }

    pub fn driven_by_scenario(&mut self) {
        self.verdict.scenario = true;
    }

    pub fn demo_path(&self) -> &std::path::Path {
        self.start.path()
    }

    pub fn past_end(&self) -> bool {
        self.past_end
    }

    pub(super) fn into_start(self) -> (Start, Verdict) {
        (self.start, self.verdict)
    }

    pub(super) fn carry(&mut self, before: Verdict) {
        self.verdict.scenario = before.scenario;
        if self.verdict.diverged.is_none() {
            self.verdict.diverged = before.diverged;
        }
    }

    fn act(&mut self, world: &mut GameWorld, t: u64, act: HostCall) {
        let console = world.console().clone();
        let failed = match act {
            HostCall::Eval(line) => world.script_manager().eval(&line).err(),
            HostCall::Call { function, args } if function.starts_with("Harness.") => {
                let verb = &function["Harness.".len()..];
                calls::invoke_harness(&self.lua, world.scene(), verb, &args).err()
            }
            HostCall::Call { function, args } if function.starts_with(script_ref::PREFIX) => {
                let verb = &function[script_ref::PREFIX.len()..];
                script_ref::invoke(world.script_manager(), verb, &args).err()
            }
            HostCall::Call { function, args } => {
                calls::invoke(&self.lua, world.script_manager(), &function, &args).err()
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

pub fn prepare(world: &mut GameWorld, demo: Demo) -> Result<(), String> {
    let lua = replay_vm(world)?;
    install(world, demo, lua);
    Ok(())
}

pub(super) fn replay_vm(world: &GameWorld) -> Result<Lua, String> {
    let lua = rusty_script::scripting::sandboxed_vm()?;
    world.script_manager().api_cells().install(&lua)?;
    Ok(lua)
}

fn install(world: &mut GameWorld, demo: Demo, lua: Lua) {
    let start = Start::of(world, demo.path());
    let header = &demo.header;
    super::sources::warn_changes(world, header);
    let mut host = Host::take(world);
    host.seed = header.seed;
    host.time.time_scale = header.time_scale.0 as f32;
    (host.time.paused, host.time.pending_steps) = (header.paused, header.pending_steps);
    host.storage.replace(header.storage.clone());
    host.video = (host.storage.get_namespace(VIDEO_NAMESPACE))
        .map_or_else(VideoSettings::default, |blob| {
            VideoSettings::from_json(&blob)
        });
    host.quality = saved_quality(&host.storage);
    let app = world.resources.application.borrow();
    host.graphics = GraphicsSettings::load(
        &host.storage,
        (app.graphics_defaults(), app.render_scale_default()),
        app.quality_presets(),
    );
    drop(app);
    host.game_view = header.size.or(host.game_view);
    host.display_scale = header.display_scale.or(host.display_scale);
    host.put(world);
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
        start,
        past_end: false,
    })));
}

impl Replayer {
    fn before_play(&mut self, world: &mut GameWorld) {
        let acts = std::mem::take(&mut self.before_play);
        if acts.iter().any(|act| matches!(act, HostCall::Eval(_))) {
            let init = world.init_edit_runtime();
            world.report_lua_init(init);
        }
        self.acts(world, 0, acts);
    }

    fn before_input(&mut self, world: &mut GameWorld) {
        let Some(tick) = &mut self.tick else {
            return;
        };
        let (t, acts, size) = (tick.t, std::mem::take(&mut tick.host), tick.size);
        let scale = tick.display_scale;
        self.acts(world, t, acts);
        let mut screen = world.resources.screen.borrow_mut();
        if let Some((width, height)) = size {
            screen.set_game_view(width, height);
        }
        if let Some(scale) = scale {
            screen.set_display_scale(scale);
        }
        drop(screen);
        if let Some(tick) = &self.tick {
            world.input().borrow_mut().set_frame(&tick.input);
        }
    }

    fn after_tick(&mut self, world: &mut GameWorld) {
        let Some(tick) = self.tick.take() else {
            self.past_end |= self.finished();
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

    fn loads(&mut self, ready: Vec<u32>) -> Vec<u32> {
        self.tick.as_ref().map_or(ready, |tick| tick.loads.clone())
    }

    fn load_progress(&mut self, moved: Option<u8>) -> Option<u8> {
        self.tick.as_ref().map_or(moved, |tick| tick.load_progress)
    }
}
