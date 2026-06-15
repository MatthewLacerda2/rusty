//! src/app/game_play.rs — play-mode transitions for [`GameWorld`].
//!
//! The enter/exit-Play lifecycle and the editor free-fly camera, split out of
//! `game.rs` to keep both files under the size cap. `handle_transition` (in
//! `game.rs`) calls into these. Entering Play snapshots the edit scene, spins up
//! the Lua runtime, loads + starts entity scripts, and builds the rapier world;
//! leaving Play tears all that down and restores the snapshot.
//!
//! The script runtime borrows the owned engine state through a `lua.scope` (#57):
//! `enter_play` builds a [`ScriptCtx`] via a disjoint-field destructure of
//! `Resources` and hands it to `start_scripts`.

use glam::Vec3;

use crate::physics::PhysicsWorld;
use crate::scene::SceneSnapshot;
use crate::scripting::ScriptCtx;

use super::{GameWorld, Resources};

impl GameWorld {
    pub(super) fn enter_play(&mut self) {
        self.resources.play_frame = 0;
        self.resources.time.reset();
        // Snapshot the authoritative edit scene so Stop can restore it, discarding
        // every play-mode mutation (script/physics moves, health, spawns/despawns).
        self.resources.edit_snapshot = Some(SceneSnapshot::capture(&self.world.scene));
        {
            let scene = &self.world.scene;
            let player = scene
                .find_entity_by_name("Player")
                .and_then(|id| scene.get_entity(id));
            if let Some(player) = player {
                let cam = &mut self.resources.camera;
                cam.position = player.transform.position + Vec3::new(0.0, 1.5, -4.5);
                cam.yaw = 90.0;
                cam.pitch = -10.0;
            }
        }
        self.resources
            .console
            .info("Capturing cursor, entering PlayMode!".to_string());

        if let Err(err) = self.resources.script_manager.init_runtime() {
            self.resources
                .console
                .error(format!("Lua init error: {}", err));
            return;
        }
        let to_load: Vec<(u32, String)> = self
            .world
            .scene
            .iter()
            .filter_map(|e| e.script.as_ref().map(|s| (e.id, s.path.clone())))
            .collect();
        for (id, path) in to_load {
            if let Err(e) = self.resources.script_manager.load_entity_script(id, &path) {
                self.resources
                    .console
                    .error(format!("Lua compile error (Entity {}): {}", id, e));
            }
        }
        {
            let (world, res) = (&mut self.world, &mut self.resources);
            let Resources {
                script_manager,
                input,
                nav,
                console,
                camera,
                time,
                physics,
                ..
            } = res;
            let mut ctx = ScriptCtx {
                scene: &mut world.scene,
                input,
                nav,
                console,
                camera,
                time,
                physics,
            };
            script_manager.start_scripts(&mut ctx);
        }

        // Build the rapier world from the (post-start) scene: bodies + colliders
        // for every entity with a ColliderComponent. Stepped each frame in play.
        // Shared with the script runtime's Physics.Raycast/Shoot bindings.
        self.resources.physics = Some(PhysicsWorld::from_scene(&self.world.scene));

        // Run the one-shot `Startup` stage now that the Play session is fully set
        // up. No built-in module registers Startup systems yet; this is the wired
        // hook modules will register into (Unity's `Start`).
        self.resources.frame_dt = 0.0;
        let schedule = std::mem::take(&mut self.resources.schedule);
        schedule.run_startup(&mut self.world, &mut self.resources);
        self.resources.schedule = schedule;
    }

    pub(super) fn exit_play(&mut self) {
        self.resources.script_manager.shutdown();
        self.resources.pathfinding_points.clear();
        self.resources.play_frame = 0;
        self.resources.physics = None;
        // Restore the edit scene captured on Play, discarding play-mode state.
        if let Some(snapshot) = self.resources.edit_snapshot.take() {
            snapshot.restore(&mut self.world.scene);
            self.resources.nav.bake(&self.world.scene);
        }
    }

    /// Editor-mode free-fly camera (no entity simulation).
    pub(super) fn editor_fly(&mut self, dt: f32) {
        let inp = &self.resources.input;
        let cam = &mut self.resources.camera;
        let mut move_dir = Vec3::ZERO;
        if inp.is_key_down("W") {
            move_dir += cam.forward();
        }
        if inp.is_key_down("S") {
            move_dir -= cam.forward();
        }
        if inp.is_key_down("A") {
            move_dir -= cam.right();
        }
        if inp.is_key_down("D") {
            move_dir += cam.right();
        }
        if move_dir.length_squared() > 0.001 {
            cam.position += move_dir.normalize() * 10.0 * dt;
        }

        let look = 90.0 * dt;
        if inp.is_key_down("LEFT") {
            cam.yaw -= look;
        }
        if inp.is_key_down("RIGHT") {
            cam.yaw += look;
        }
        if inp.is_key_down("UP") {
            cam.pitch += look;
        }
        if inp.is_key_down("DOWN") {
            cam.pitch -= look;
        }
        cam.pitch = cam.pitch.clamp(-80.0, 80.0);
    }
}
