use super::resources::Resources;
use super::schedule::Schedule;
use super::stage::Stage;
use super::world::World;

#[derive(Default)]
pub struct App {
    schedule: Schedule,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    /// Order within a stage is call order.
    pub fn add_system<F>(&mut self, stage: Stage, system: F) -> &mut Self
    where
        F: Fn(&mut World, &mut Resources) + 'static,
    {
        self.schedule.add_system(stage, system);
        self
    }

    pub fn into_schedule(self) -> Schedule {
        self.schedule
    }
}

/// Build the default engine `App`; its registration order is the per-frame system order.
pub fn build() -> App {
    let mut app = App::new();
    super::play::register(&mut app);
    super::audio::register(&mut app);
    super::systems::ui::register(&mut app);
    app
}
