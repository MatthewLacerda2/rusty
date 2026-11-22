use super::probe::SystemProbe;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;

type BoxedSystem = Box<dyn Fn(&mut World, &mut Resources)>;

struct Entry {
    name: &'static str,
    run: BoxedSystem,
}

#[derive(Default)]
pub struct Schedule {
    stages: [Vec<Entry>; Stage::COUNT],
}

impl Schedule {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_system<F>(&mut self, stage: Stage, system: F)
    where
        F: Fn(&mut World, &mut Resources) + 'static,
    {
        let name = short_name(std::any::type_name::<F>());
        self.stages[stage.index()].push(Entry {
            name,
            run: Box::new(system),
        });
    }

    #[cfg(test)]
    pub fn system_names(&self, stage: Stage) -> Vec<&'static str> {
        self.stages[stage.index()].iter().map(|e| e.name).collect()
    }

    pub fn run_startup(&self, world: &mut World, res: &mut Resources) {
        self.run_stage(Stage::Startup, world, res, &mut None);
    }

    /// An installed probe is moved out of `res` for the frame so systems can take
    /// `&mut Resources`.
    pub fn run_frame(&self, world: &mut World, res: &mut Resources) {
        let mut probe = res.probe.take();
        for stage in Stage::FRAME_ORDER {
            self.run_stage(stage, world, res, &mut probe);
        }
        if let Some(probe) = probe.as_mut() {
            probe.end_frame(world, res);
        }
        res.probe = probe;
    }

    fn run_stage(
        &self,
        stage: Stage,
        world: &mut World,
        res: &mut Resources,
        probe: &mut Option<Box<dyn SystemProbe>>,
    ) {
        for entry in &self.stages[stage.index()] {
            match probe.as_mut() {
                Some(p) => p.run(stage, entry.name, &mut || (entry.run)(world, res)),
                None => (entry.run)(world, res),
            }
        }
        // Stores the AABBs the stage's Transform writes queued. `World::collider_bounds`
        // never depends on it, so a scene borrowed elsewhere just waits for the next stage.
        let mut flush = || {
            if let Ok(mut scene) = world.scene.try_borrow_mut() {
                scene.world.flush_bounds();
            }
        };
        match probe.as_mut() {
            Some(p) => p.run(stage, "flush_bounds", &mut flush),
            None => flush(),
        }
    }
}

/// `rusty::app::play::tick_nav` → `tick_nav`. A non-item (a closure or a bare fn
/// pointer) has no useful path, so it reports as `system`.
fn short_name(type_name: &'static str) -> &'static str {
    if type_name.contains(['(', '{', '<']) {
        return "system";
    }
    type_name.rsplit("::").next().unwrap_or(type_name)
}
