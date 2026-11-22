use super::game::GameWorld;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;
use rusty_script::scripting::ConsoleLogs;

/// Must run `system` exactly once per call: a probe watches, never changes, the schedule.
pub trait SystemProbe {
    fn run(&mut self, stage: Stage, name: &'static str, system: &mut dyn FnMut());

    /// Inside the tick's span: a timing probe leaves its own time here out of the tick's.
    fn end_frame(&mut self, world: &World, res: &Resources);

    /// Run `part` exactly once: work in the tick's span that a shipped windowed game
    /// does not do (`taps`, `loads`), timed beside the tick, not in it.
    fn run_part(&mut self, _name: &'static str, part: &mut dyn FnMut()) {
        part();
    }

    /// After the tick's play transition: entering Play is timed by the entry probe, not as a tick.
    fn begin_tick(&mut self) {}

    fn end_tick(&mut self) {}
}

pub trait LoadProbe {
    fn step(&mut self, name: &'static str, step: &mut dyn FnMut());

    /// `loaded` is `None` when nothing loaded: the scene load or the scripts' runtime failed.
    fn finish(&mut self, loaded: Option<&str>, console: &mut ConsoleLogs);
}

#[derive(Default)]
pub struct LoadProbes {
    pub(super) scene: Option<Box<dyn LoadProbe>>,
    pub(super) entry: Option<Box<dyn LoadProbe>>,
}

pub fn timed<T>(
    probe: &mut Option<Box<dyn LoadProbe>>,
    name: &'static str,
    step: impl FnOnce() -> T,
) -> T {
    let Some(probe) = probe.as_mut() else {
        return step();
    };
    let (mut step, mut out) = (Some(step), None);
    probe.step(name, &mut || {
        if let Some(run) = step.take() {
            out = Some(run());
        }
    });
    match (out, step) {
        (Some(out), _) => out,
        (None, Some(run)) => run(),
        (None, None) => unreachable!("a step that ran left its result"),
    }
}

impl GameWorld {
    pub(super) fn probe_tick(&mut self, begin: bool) {
        if let Some(p) = self.resources.probe.as_mut() {
            match begin {
                true => p.begin_tick(),
                false => p.end_tick(),
            }
        }
    }

    pub(super) fn tap_timed(&mut self, point: super::TapPoint) {
        if self.resources.tap.is_none() {
            return;
        }
        let mut probe = self.resources.probe.take();
        match probe.as_mut() {
            Some(p) => p.run_part("taps", &mut || self.tap(point)),
            None => self.tap(point),
        }
        if let Some(p) = probe {
            self.resources.probe.get_or_insert(p);
        }
    }

    pub(super) fn take_load(
        &mut self,
        op: u32,
    ) -> Result<rusty_edit::load::prepare::PreparedScene, String> {
        let res = &mut self.resources;
        let loader = &mut res.loading.loader;
        match res.probe.as_mut().filter(|_| loader.prepares_inline()) {
            Some(p) => {
                let mut taken = Err(String::new());
                p.run_part("loads", &mut || taken = loader.take(op));
                taken
            }
            None => loader.take(op),
        }
    }

    pub fn set_probe(&mut self, probe: Option<Box<dyn SystemProbe>>) {
        self.resources.probe = probe;
    }

    pub fn set_load_probe(&mut self, probe: Option<Box<dyn LoadProbe>>) {
        self.resources.load_probes.scene = probe;
    }

    pub fn set_entry_probe(&mut self, probe: Option<Box<dyn LoadProbe>>) {
        self.resources.load_probes.entry = probe;
    }
}
