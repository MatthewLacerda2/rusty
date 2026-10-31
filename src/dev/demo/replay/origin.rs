//! Where a world goes back to after a replay played on it (#1598): one restore,
//! shared by a seek back to the demo's start (`seek`) and the editor closing a
//! demo over its edit scene (`shell::editor::demo`), so the two cannot drift.
//!
//! The scene comes back whole: its entities, its navmesh, its identity (so a
//! Generate Lighting waiting on it still recognises it) and the file text it last
//! read or wrote (so the editor's own next write is no outside change to reload).
//! The live input and the UI's event state are reset, since the replay wrote the
//! recorded ones every tick: a key held at the last tick replayed is not held now.

use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::io::DiskText;
use crate::scene::{SceneId, SceneSnapshot};

/// A world's scene as it stood, to put back.
pub struct Origin {
    snapshot: SceneSnapshot,
    id: SceneId,
    disk_text: DiskText,
}

impl Origin {
    /// `world`'s scene as it stands now.
    pub fn of(world: &GameWorld) -> Self {
        let scene = world.scene().borrow();
        Self {
            snapshot: SceneSnapshot::capture(&scene),
            id: scene.id(),
            disk_text: scene.disk_text(),
        }
    }

    /// Put the scene back into `world`, out of Play, with no input held, no UI
    /// event pending and no quit asked. Play is left first when it is on.
    pub fn restore(self, world: &mut GameWorld) {
        world.set_playing(false);
        world.poll_transition();
        // A demo that ended on `Application.Quit()` starts again unquit.
        world.take_quit_request();
        let mut scene = world.scene().borrow_mut();
        self.snapshot.restore(&mut scene);
        scene.restore_id(self.id);
        scene.restore_disk_text(self.disk_text);
        *world.nav().borrow_mut() = NavigationGraph::from_scene(&scene);
        drop(scene);
        *world.input().borrow_mut() = InputState::new();
        *world.resources.event_system.borrow_mut() = Default::default();
    }
}
