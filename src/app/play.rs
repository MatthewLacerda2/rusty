//! src/app/play.rs — play-mode systems.
//!
//! Each free function is an engine "system" operating on the `GameWorld`. There is
//! NO gameplay here: no control scheme, no weapon, no damage constants. The player
//! controller and the weapon are bundled GAME scripts
//! (`assets/scripts/player_controller.lua`, `bot.lua`) attached to entities; the
//! systems below only run engine logic (nav, physics, scripts, animator). No
//! system looks an entity up by name (#450): where the play camera starts and what
//! any entity does are scene and script decisions.
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

use super::game::GameWorld;
use super::registry::App;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;

/// Play-enter helpers, split out of `GameWorld::enter_play` so each phase stays
/// small and named. These run once when the world transitions into Play.
impl GameWorld {
    /// Boot straight into Play, the way the standalone player runs a game (#431):
    /// no edit snapshot is taken (there is no edit mode to return to) and Play is
    /// requested at once, so the next `tick` enters it and reports
    /// [`PlayTransition::Entered`](super::PlayTransition::Entered) like the editor's
    /// Play button would.
    pub fn boot_standalone(&mut self) {
        self.resources.standalone = true;
        self.set_playing(true);
    }

    /// Whether the game called `Application.Quit()` and no host has consumed it yet.
    pub fn quit_requested(&self) -> bool {
        self.resources.application.borrow().quit_requested()
    }

    /// Consume a pending `Application.Quit()`: `true` exactly once per call. The host
    /// decides what it means (the player exits, the editor stops Play).
    pub fn take_quit_request(&mut self) -> bool {
        self.resources.application.borrow_mut().take_quit_request()
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
/// same tick). `late_update_scripts` (#324) runs after physics, animation and
/// particles have resolved this tick's state, so scripts can react to settled
/// transforms. Trails (#441) record right after it, so they sample where
/// `LateUpdate` left each entity. `apply_destroys` (#323) is the tick's tail: it drains the
/// deferred-destroy queue (firing `OnDisable`/`OnDestroy`) after every other
/// system has seen the entity. The scene-load phase (#432) follows it, so a
/// `Scene.Load` swaps the World only once the whole tick is done with it, right
/// before `advance_frame`. The skin palette build (#453) runs in `Render`, after
/// every stage that may move a bone.
pub(super) fn register(app: &mut App) {
    app.add_system(Stage::FixedUpdate, rebake_nav)
        .add_system(Stage::FixedUpdate, init_scripts)
        .add_system(Stage::FixedUpdate, super::ui::dispatch_ui_events)
        .add_system(Stage::FixedUpdate, update_scripts)
        .add_system(Stage::FixedUpdate, tick_nav)
        .add_system(Stage::FixedUpdate, step_physics)
        .add_system(Stage::FixedUpdate, super::animation::graph::evaluate_graphs)
        .add_system(Stage::FixedUpdate, animate)
        .add_system(Stage::FixedUpdate, super::particles::tick_particles)
        .add_system(Stage::FixedUpdate, late_update_scripts)
        .add_system(Stage::FixedUpdate, super::trails::tick_trails)
        .add_system(Stage::FixedUpdate, apply_destroys)
        .add_system(Stage::FixedUpdate, super::scene_load::apply_scene_load)
        .add_system(Stage::FixedUpdate, advance_frame)
        .add_system(Stage::Render, build_skin_palettes);
}

/// The script phase's head: drain queued script loads — entities spawned during
/// play get their scripts compiled here, one tick after the spawn — and run every
/// pending `Awake` then `Start` (#322), so init always precedes the tick's UI
/// callbacks (#420) and any `Update`.
fn init_scripts(_world: &mut World, res: &mut Resources) {
    res.script_manager.init_scripts();
}

/// The script phase's body: every started script's `Update`, after this tick's
/// UI callbacks, so gameplay reads `UI.IsPointerConsumed` already settled. Then
/// the timer phase (#444): due `Timer.Invoke`s fire and waiting coroutines resume,
/// on the same scaled `dt` — after every `Update`, before physics and `LateUpdate`.
fn update_scripts(_world: &mut World, res: &mut Resources) {
    res.script_manager.update_scripts(res.frame_dt);
    res.script_manager.tick_timers(res.frame_dt);
}

/// The post-physics script phase (#324): once physics, animation and particles
/// have resolved this tick's state, every started script's `LateUpdate(id, dt)`
/// runs — still inside the deterministic `FixedUpdate` stage, before
/// `advance_frame`. A follow-cam / look-at / aim / recoil script placed here reads
/// *this* tick's settled transforms instead of last tick's, closing the one-step
/// lag. It reuses the same scaled `frame_dt` the tick handed `update_scripts`.
fn late_update_scripts(_world: &mut World, res: &mut Resources) {
    res.script_manager.late_update_scripts(res.frame_dt);
}

/// The destroy phase (#323): drain the deferred-destroy queue that play-mode
/// `Scene.DestroyEntity` calls filled this tick, firing each doomed entity's
/// `OnDisable` (if it was active) then `OnDestroy` before the entity — and its
/// script instances — are removed. Registered at the tick's tail, after
/// `late_update_scripts` and before `advance_frame`, so a destroyed entity still
/// took part in this whole tick (Unity's end-of-frame `Object.Destroy`).
fn apply_destroys(_world: &mut World, res: &mut Resources) {
    res.script_manager.apply_pending_destroys();
}

/// Step the rapier world and dispatch the resulting trigger and collision
/// events to scripts.
fn step_physics(world: &mut World, res: &mut Resources) {
    let dt = res.frame_dt;
    let events = {
        let mut s = world.scene.borrow_mut();
        match res.physics.borrow_mut().as_mut() {
            Some(physics) => physics.step(&mut s, dt),
            None => crate::physics::PhysicsEvents::default(),
        }
    };
    res.script_manager.dispatch_physics_events(events);
}

/// Advance the deterministic play-mode frame counter (drives the rebake cadence).
fn advance_frame(_world: &mut World, res: &mut Resources) {
    res.play_frame += 1;
}

/// Rebake the navmesh once per second (every `REBAKE_INTERVAL_FRAMES` frames), so
/// the graph follows static geometry that moved during play.
fn rebake_nav(world: &mut World, res: &mut Resources) {
    if !res.play_frame.is_multiple_of(REBAKE_INTERVAL_FRAMES) {
        return;
    }
    res.nav.borrow_mut().bake(&world.scene.borrow());
}

fn tick_nav(world: &mut World, res: &mut Resources) {
    let mut s = world.scene.borrow_mut();
    let nav = res.nav.borrow();
    nav.tick_nav_agents(&mut s, res.frame_dt);
}

/// Advance every active entity's animator and write the sampled pose onto its
/// bone GameObjects (#80, #453). The sampler is a pure function of (skin, clip,
/// time), so stepping at the fixed dt is deterministic. Writers after this one
/// (`LateUpdate` scripts, physics) may override a bone before the palette build.
fn animate(world: &mut World, res: &mut Resources) {
    let dt = res.frame_dt;
    let mut s = world.scene.borrow_mut();
    for id in s.world.ids_with_animator() {
        if !s.world.is_active(id) {
            continue;
        }
        // The animator + mesh split borrow goes through the facade's sanctioned
        // helper (#344) — the animate system's re-pose step.
        let writes = s.world.with_animator_and_mesh_mut(id, |anim, mesh| {
            // The current clip's duration is the wrap length when the animator loops.
            let duration = super::animation::current_clip_duration(anim, mesh.as_deref());
            anim.advance(dt, duration);
            mesh.map(|m| super::animation::bone_writes(anim, m))
        });
        for (bone, local) in writes.flatten().unwrap_or_default() {
            if let Some(mut t) = s.world.transform_mut(bone) {
                *t = local;
            }
        }
    }
}

/// The palette build (#453): once every pose writer of the frame has run, skin
/// each mesh from its bones. Registered in `Render` — draw-data preparation, after
/// `LateUpdate`; edit mode calls the same build from `GameWorld::tick`.
fn build_skin_palettes(world: &mut World, _res: &mut Resources) {
    world.scene.borrow_mut().build_skin_palettes();
}

#[cfg(test)]
#[path = "standalone_tests.rs"]
mod standalone_tests;

#[cfg(test)]
#[path = "play_tests.rs"]
mod play_tests;
