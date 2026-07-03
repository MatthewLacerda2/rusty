//! src/app/play.rs — play-mode systems.
//!
//! Each free function is an engine "system" operating on the `GameWorld`. There is
//! NO gameplay here: no control scheme, no weapon, no damage constants. The player
//! controller and the weapon are bundled GAME scripts
//! (`assets/scripts/player_controller.lua`, `bot.lua`) attached to entities; the
//! systems below only run engine logic (nav, physics, scripts, animator). Entity
//! lookups for the debug nav path still go through the name map (the demo's
//! "Player" / "Enemy_1"); that is debug visualisation, not gameplay.
//!
//! These systems are no longer called by a hand-wired sequence: [`register`]
//! pushes them into the schedule's `FixedUpdate` stage, in the order they used to
//! run, and `GameWorld::tick` drives the schedule. `FixedUpdate` is the
//! deterministic fixed-dt stage the headless harness steps, so every simulation
//! system belongs there; the per-frame order is unchanged.
//!
//! Each system is the canonical `fn(&mut World, &mut Resources)` (#39): the scene
//! is reached through `world.scene` (a short-lived borrow, so re-entrant scripts can
//! borrow it too), the engine singletons through `res`, and the scaled per-frame
//! delta through `res.dt()`.

use glam::Vec3;

use super::game::GameWorld;
use super::registry::App;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;

const PLAYER_NAME: &str = "Player";
const ENEMY_NAME: &str = "Enemy_1";

/// Play-enter helpers, split out of `GameWorld::enter_play` so each phase stays
/// small and named. These run once when the world transitions into Play.
impl GameWorld {
    /// Place the play-mode camera just behind the Player entity (no-op if absent).
    pub(super) fn snap_camera_to_player(&mut self) {
        let scene = self.world.scene.borrow();
        let player = scene
            .find_entity_by_name(PLAYER_NAME)
            .and_then(|id| scene.get_entity(id));
        if let Some(player) = player {
            let mut cam = self.resources.camera.borrow_mut();
            cam.position = player.transform.position + Vec3::new(0.0, 1.5, -4.5);
            cam.yaw = 90.0;
            cam.pitch = -10.0;
        }
    }

    /// Run the one-shot `Startup` stage now that the Play session is fully set
    /// up. No built-in module registers Startup systems yet; this is the wired
    /// hook modules will register into (Unity's `Start`).
    pub(super) fn run_startup_stage(&mut self) {
        self.resources.frame_dt = 0.0;
        let schedule = std::mem::take(&mut self.resources.schedule);
        schedule.run_startup(&mut self.world, &mut self.resources);
        self.resources.schedule = schedule;
    }
}

/// Rebake cadence in play-mode frames. At the fixed 1/60 timestep this is "once a
/// second", but the trigger is the frame count, not the wall clock — that is what
/// makes a headless replay deterministic.
const REBAKE_INTERVAL_FRAMES: u64 = 60;

/// Register the play-mode systems into the schedule, in the exact order the old
/// hand-wired loop ran them. All are sim systems, so they live in `FixedUpdate`.
/// The graph evaluator (#316) sits after the scripts (so parameters set this tick
/// are seen) and right before `animate` (so a fired transition is sampled the
/// same tick).
pub(super) fn register(app: &mut App) {
    app.add_system(Stage::FixedUpdate, rebake_and_path)
        .add_system(Stage::FixedUpdate, update_scripts)
        .add_system(Stage::FixedUpdate, tick_nav)
        .add_system(Stage::FixedUpdate, step_physics)
        .add_system(Stage::FixedUpdate, super::animation::graph::evaluate_graphs)
        .add_system(Stage::FixedUpdate, animate)
        .add_system(Stage::FixedUpdate, super::particles::tick_particles)
        .add_system(Stage::FixedUpdate, advance_frame);
}

/// The script phase. Its head drains queued script loads — entities spawned
/// during play get their scripts compiled here, one tick after the spawn — and
/// runs every pending `Awake` then `Start` (#322), so init always precedes any
/// `Update` of the tick. Then every started script's `Update` runs.
fn update_scripts(_world: &mut World, res: &mut Resources) {
    res.script_manager.init_scripts();
    res.script_manager.update_scripts(res.frame_dt);
}

/// Step the rapier world and dispatch any resulting trigger events to scripts.
fn step_physics(world: &mut World, res: &mut Resources) {
    let dt = res.frame_dt;
    let events = {
        let mut s = world.scene.borrow_mut();
        match res.physics.borrow_mut().as_mut() {
            Some(physics) => physics.step(&mut s, dt),
            None => crate::physics::TriggerEvents::default(),
        }
    };
    if !events.is_empty() {
        res.script_manager.dispatch_trigger_events(events);
    }
}

/// Advance the deterministic play-mode frame counter (drives the rebake cadence).
fn advance_frame(_world: &mut World, res: &mut Resources) {
    res.play_frame += 1;
}

/// Rebake the navmesh once per second (every `REBAKE_INTERVAL_FRAMES` frames) and
/// recompute the Enemy→Player debug path.
fn rebake_and_path(world: &mut World, res: &mut Resources) {
    if !res.play_frame.is_multiple_of(REBAKE_INTERVAL_FRAMES) {
        return;
    }
    let s = world.scene.borrow();
    res.nav.borrow_mut().bake(&s);
    let enemy_pos = s
        .find_entity_by_name(ENEMY_NAME)
        .and_then(|id| s.get_entity(id))
        .map(|e| e.transform.position);
    let player_pos = s
        .find_entity_by_name(PLAYER_NAME)
        .and_then(|id| s.get_entity(id))
        .map(|e| e.transform.position);
    if let (Some(enemy_pos), Some(player_pos)) = (enemy_pos, player_pos) {
        let grid = res.nav.borrow();
        let (es_x, es_z) = grid.world_to_grid(enemy_pos);
        let (pl_x, pl_z) = grid.world_to_grid(player_pos);
        let mut pts = vec![enemy_pos];
        if let Some(grid_pts) = grid.find_path(es_x, es_z, pl_x, pl_z) {
            for &(gx, gz) in &grid_pts {
                pts.push(grid.grid_to_world(gx, gz));
            }
        }
        pts.push(player_pos);
        res.pathfinding_points = pts;
    }
}

fn tick_nav(world: &mut World, res: &mut Resources) {
    let mut s = world.scene.borrow_mut();
    let nav = res.nav.borrow();
    nav.tick_nav_agents(&mut s, res.frame_dt);
}

/// Advance every active entity's animator and re-pose its skinned mesh from the
/// imported clips (#80). The sampler is a pure function of (skin, clip, time), so
/// stepping at the fixed dt is deterministic.
fn animate(world: &mut World, res: &mut Resources) {
    let dt = res.frame_dt;
    let mut s = world.scene.borrow_mut();
    for id in s.entity_ids() {
        let Some(mut entity) = s.get_entity_mut(id) else {
            continue;
        };
        if !entity.active {
            continue;
        }
        // Split the borrow: `animator` and `mesh` are distinct fields of `entity`,
        // so destructure to mutate both without aliasing.
        let crate::components::Entity { animator, mesh, .. } = &mut *entity;
        let Some(anim) = animator else {
            continue;
        };
        // The current clip's duration is the wrap length when the animator loops.
        let duration = super::animation::current_clip_duration(anim, mesh.as_ref());
        anim.advance(dt, duration);
        if let Some(mesh) = mesh {
            super::animation::repose_mesh(anim, mesh);
        }
    }
}
