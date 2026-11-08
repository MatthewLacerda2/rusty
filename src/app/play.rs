//! src/app/play.rs — play-mode systems.
//!
//! Each free function is an engine "system" operating on the `GameWorld`. There is
//! NO gameplay here: no control scheme, no weapon, no damage constants. The player
//! controller and the weapon are bundled GAME scripts
//! (`engine/scripts/player_controller.lua`, `bot.lua`) attached to entities; the
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
//! borrow it too), the engine singletons through `res`, and the tick's scaled delta
//! through `res.frame_dt` (the fixed dt × time scale in every host's Play, #1877).

use super::game::GameWorld;
use super::registry::App;
use super::resources::Resources;
use super::stage::Stage;
use super::systems::physics;
use super::world::World;

/// Play-enter helpers, split out of `GameWorld::enter_play` so each phase stays
/// small and named. These run once when the world transitions into Play.
impl GameWorld {
    /// Boot straight into Play, the way the standalone player runs a game (#431):
    /// no edit snapshot is taken (there is no edit mode to return to) and Play is
    /// requested at once, so the next `tick` enters it and reports
    /// [`PlayTransition::Entered`](super::PlayTransition::Entered) like the editor's
    /// Play button would. Entering it refreshes the project's converted models and
    /// then holds conversion off, as the editor's Play does (#1253).
    pub fn boot_standalone(&mut self) {
        crate::asset::convert::mark_standalone();
        self.resources.application.borrow_mut().mark_standalone();
        self.set_playing(true);
    }

    /// Bring the project's converted sources up to date and the scene's instances
    /// with them, then refuse conversion for the session (#1253), as Unity refreshes
    /// its assets before Play and its runtime loads never import. Runs before the
    /// edit snapshot, so Stop restores the refreshed scene. The standalone player
    /// does the same; on an exported project (#1267) there are no sources to refresh.
    pub(super) fn refresh_then_hold_imports(&mut self) {
        use crate::asset::convert::{refresh_sources, PlayHold};
        refresh_sources(&self.resources.sources_root);
        let mut scene = self.world.scene.borrow_mut();
        self.resources.model_writes.follow(&mut scene);
        self.resources.play_hold = Some(PlayHold::take());
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
    /// up: the hook modules register into (Unity's `Start`); the audio module's
    /// play-on-start is one.
    pub(super) fn run_startup_stage(&mut self) {
        self.resources.frame_dt = 0.0;
        let schedule = std::mem::take(&mut self.resources.schedule);
        schedule.run_startup(&mut self.world, &mut self.resources);
        self.resources.schedule = schedule;
    }
}

/// Register the play-mode systems into the schedule, in the exact order the old
/// hand-wired loop ran them. The sim systems live in `FixedUpdate`.
/// The graph evaluator (#316) sits after the scripts (so parameters set this tick
/// are seen) and right before `animate` (so a fired transition is sampled the
/// same tick). `late_update_scripts` (#324) runs after physics, animation and
/// particles have resolved this tick's state, so scripts can react to settled
/// transforms. Trails (#441) record after it and the post-animation writers below,
/// so they sample where `LateUpdate` left each entity. `apply_destroys` (#323) is
/// the tick's tail: it drains the deferred-destroy queue (firing `OnDisable` /
/// `OnDestroy`) after every other system has seen the entity; decals age right
/// after it (#639), so a destroyed owner's decals go the same tick. The scene-load
/// phase (#432) follows it, so a `Scene.Load` swaps the World only once the whole
/// tick is done with it, right before `advance_frame`. The skin palette build (#453)
/// runs in `Render`, after every stage that may move a bone. The post-animation
/// writers, in order: `animate` writes the clip pose; `pose_ragdolls` (#466) puts each simulated
/// ragdoll bone back where physics left it; the state callbacks (#1901) and
/// `OnAnimationEvent` (#459) then fire for the states and events of this tick's step;
/// `perceive` (#1900) then steps every `AIPerception`'s sight against this tick's
/// colliders and settled pose, and `OnPerceptionUpdated` fires for what it and
/// this tick's noises and hits raised; `LateUpdate` scripts set this tick's
/// targets; `solve_ik` (#461) post-processes the final pose, skipping any bone a
/// dynamic body carries (physics wins over IK); `follow_bones` (#464) then moves
/// the hitbox query poses to the pose that renders; `carry_local_particles` (#1529)
/// then moves a `Local` emitter's particles to where all of that left the emitter.
pub(super) fn register(app: &mut App) {
    app.add_system(Stage::FixedUpdate, rebake_nav)
        .add_system(Stage::FixedUpdate, init_scripts)
        .add_system(Stage::FixedUpdate, super::systems::ui::dispatch_ui_events)
        .add_system(Stage::FixedUpdate, update_scripts)
        .add_system(Stage::FixedUpdate, tick_nav)
        .add_system(Stage::FixedUpdate, physics::step_physics)
        .add_system(Stage::FixedUpdate, super::animation::graph::evaluate_graphs)
        .add_system(Stage::FixedUpdate, super::systems::animate::animate)
        .add_system(Stage::FixedUpdate, physics::pose_ragdolls)
        .add_system(Stage::FixedUpdate, dispatch_animation_events)
        .add_system(Stage::FixedUpdate, perceive)
        .add_system(Stage::FixedUpdate, dispatch_perception_updates)
        .add_system(
            Stage::FixedUpdate,
            super::systems::particles::tick_particles,
        )
        .add_system(Stage::FixedUpdate, late_update_scripts)
        .add_system(Stage::FixedUpdate, solve_ik)
        .add_system(Stage::FixedUpdate, physics::follow_bones)
        .add_system(
            Stage::FixedUpdate,
            super::systems::particles::carry_local_particles,
        )
        .add_system(Stage::FixedUpdate, super::systems::trails::tick_trails)
        .add_system(Stage::FixedUpdate, apply_destroys)
        .add_system(Stage::FixedUpdate, super::systems::decals::tick_decals)
        .add_system(Stage::FixedUpdate, super::scene_load::apply_scene_load)
        .add_system(Stage::FixedUpdate, advance_frame)
        .add_system(Stage::Render, build_skin_palettes)
        .add_system(Stage::Startup, reset_sight_rays);
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
    let removed = res.script_manager.apply_pending_destroys();
    let tick = res.play_frame();
    res.audio.borrow_mut().stop_sources(removed, tick);
}

/// Bend the posed bones toward their IK targets (#461, `scene::skeleton::ik`):
/// after the Animator and `LateUpdate`, before `follow_bones` and the palette
/// build. A ragdolled bone (dynamic body) is skipped — physics wins.
fn solve_ik(world: &mut World, _res: &mut Resources) {
    world.scene.borrow_mut().solve_ik();
}

/// Advance the deterministic play-mode frame counter.
fn advance_frame(_world: &mut World, res: &mut Resources) {
    res.play_frame += 1;
}

/// Keep the navmesh in step with the scene every tick (#456): advance the
/// obstacles' carving bookkeeping, then rebake only the cells whose static
/// colliders or carving obstacles changed; a tick no static entity changed in
/// skips the static colliders altogether (#1707). Play frame 0 starts afresh from the
/// scene's saved navmesh, or bakes in full when it has none that matches (#1887), so a
/// Play session starts from the same navmesh whatever edit mode left behind; a
/// play-mode `Scene.Load` starts its scene the same way, in the load step.
fn rebake_nav(world: &mut World, res: &mut Resources) {
    let mut scene = world.scene.borrow_mut();
    crate::navigation::tick_obstacles(&mut scene, res.frame_dt);
    let mut nav = res.nav.borrow_mut();
    if res.play_frame == 0 || !nav.tracks(&scene) {
        nav.start_tracked(&mut scene);
    } else {
        nav.sync_changes(&mut scene);
    }
}

fn tick_nav(world: &mut World, res: &mut Resources) {
    let mut s = world.scene.borrow_mut();
    let nav = res.nav.borrow();
    nav.tick_nav_agents(&mut s, res.frame_dt);
}

/// `OnStateExit` / `OnStateEnter` / `OnStateUpdate` (#1901), then
/// `OnAnimationEvent(id, name, info)` (#459) for every event `animate` queued this
/// tick, so a state's first marker follows its enter: after the animation step and ragdoll posing, so a handler reads the
/// pose this tick renders (bar `LateUpdate` and IK), and before `LateUpdate`, as
/// in Unity, where animation events fire during the Animator's update. Then the
/// `OnControllerColliderHit`s of the moves root motion made (#1879): a script's
/// own `Move` raises its hits inside the call, but root motion moves outside any
/// script, so its hits wait for this point, the animation step's callbacks.
fn dispatch_animation_events(_world: &mut World, res: &mut Resources) {
    let queued = std::mem::take(&mut res.animation_events);
    res.script_manager.dispatch_state_callbacks(&queued.states);
    res.script_manager.dispatch_animation_events(&queued.events);
    let hits = std::mem::take(&mut res.controller_hits);
    res.script_manager.dispatch_queued_controller_hits(&hits);
}

/// The perception step (#1900): sight for every active `AIPerception`, on the
/// round-robin ray budget, then each memory ages by the tick's scaled dt. After
/// physics and the post-animation pose, so the rays see where things stand now.
fn perceive(world: &mut World, res: &mut Resources) {
    let physics = res.physics.borrow();
    let mut scene = world.scene.borrow_mut();
    crate::ai::perception::step(
        &mut scene,
        physics.as_ref(),
        &mut res.sight_rays,
        res.frame_dt,
    );
}

/// `OnPerceptionUpdated(id, target, info)` (#1900) for every update the step, a
/// noise or a hit raised since the last dispatch: after `perceive`, before
/// `LateUpdate`, perceivers in id order.
fn dispatch_perception_updates(world: &mut World, res: &mut Resources) {
    let updates = crate::ai::perception::drain_updates(&mut world.scene.borrow_mut());
    res.script_manager.dispatch_perception_updates(&updates);
}

/// Every Play session starts the sight round-robin from the first pair (#1900),
/// so two runs of the same scene trace the same pairs on the same ticks. (The
/// perceivers' memories were emptied on Play entry, `Scene::set_playing`.)
fn reset_sight_rays(_world: &mut World, res: &mut Resources) {
    res.sight_rays.reset();
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

#[cfg(test)]
#[path = "play_navmesh_tests.rs"]
mod play_navmesh_tests;

#[cfg(test)]
#[path = "import_tests/mod.rs"]
mod import_tests;
