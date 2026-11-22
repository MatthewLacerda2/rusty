use rusty_app::GameWorld;
use rusty_core::input::InputState;
use rusty_edit::SceneSnapshot;
use rusty_navigation::NavigationGraph;

/// One restore shared by a seek back (`seek`) and the editor closing a demo
/// (`editor::demo`), so the two cannot drift. `SceneSnapshot`, as Stop restores, keeps the
/// scene's identity and file text, so a waiting bake still recognises it.
pub struct Origin {
    snapshot: SceneSnapshot,
}

impl Origin {
    pub fn of(world: &GameWorld) -> Self {
        Self {
            snapshot: SceneSnapshot::capture(&world.scene().borrow()),
        }
    }

    pub fn restore(self, world: &mut GameWorld) {
        world.set_playing(false);
        world.poll_transition();
        world.take_quit_request();
        let mut scene = world.scene().borrow_mut();
        self.snapshot.restore(&mut scene);
        *world.nav().borrow_mut() = NavigationGraph::from_scene(&scene);
        drop(scene);
        // The replay wrote the recorded input every tick: a key held at its last stays held.
        *world.input().borrow_mut() = InputState::new();
        *world.resources.event_system.borrow_mut() = Default::default();
    }
}
