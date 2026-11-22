use super::game::GameWorld;
use super::registry::App;
use super::resources::Resources;
use super::stage::Stage;
use super::systems::{perception, physics};
use super::world::World;

impl GameWorld {
    pub fn boot_standalone(&mut self) {
        rusty_asset::convert::mark_standalone();
        self.resources.application.borrow_mut().mark_standalone();
        self.set_playing(true);
    }

    /// Before the edit snapshot, so Stop restores the refreshed scene.
    pub(super) fn refresh_then_hold_imports(&mut self) {
        use rusty_asset::convert::{refresh_sources, PlayHold};
        refresh_sources(&self.resources.sources_root);
        let mut scene = self.world.scene.borrow_mut();
        self.resources.model_writes.follow(&mut scene);
        self.resources.play_hold = Some(PlayHold::take());
    }

    pub(super) fn build_physics(&mut self) {
        let world = rusty_physics::PhysicsWorld::from_scene(&self.world.scene.borrow());
        *self.resources.physics.borrow_mut() = Some(world);
        save_hulls();
    }

    pub(super) fn drop_physics(&mut self) {
        *self.resources.physics.borrow_mut() = None;
        save_hulls();
    }

    pub fn quit_requested(&self) -> bool {
        self.resources.application.borrow().quit_requested()
    }

    pub fn take_quit_request(&mut self) -> bool {
        self.resources.application.borrow_mut().take_quit_request()
    }

    pub(super) fn run_startup_stage(&mut self) {
        self.resources.frame_dt = 0.0;
        let schedule = std::mem::take(&mut self.resources.schedule);
        schedule.run_startup(&mut self.world, &mut self.resources);
        self.resources.schedule = schedule;
    }
}

/// Order matters: the graph evaluator right before `animate`; `LateUpdate` scripts after
/// physics and animation; `apply_destroys` at the tick's tail, then the scene load.
pub(super) fn register(app: &mut App) {
    app.add_system(Stage::FixedUpdate, age_debug_lines)
        .add_system(Stage::FixedUpdate, rebake_nav)
        .add_system(Stage::FixedUpdate, init_scripts)
        .add_system(Stage::FixedUpdate, super::systems::ui::dispatch_ui_events)
        .add_system(Stage::FixedUpdate, update_scripts)
        .add_system(Stage::FixedUpdate, tick_script_timers)
        .add_system(Stage::FixedUpdate, serve_queries)
        .add_system(Stage::FixedUpdate, tick_nav)
        .add_system(Stage::FixedUpdate, physics::step_physics)
        .add_system(Stage::FixedUpdate, super::animation::graph::evaluate_graphs)
        .add_system(Stage::FixedUpdate, super::systems::animate::animate)
        .add_system(Stage::FixedUpdate, physics::pose_ragdolls)
        .add_system(Stage::FixedUpdate, dispatch_animation_events)
        .add_system(Stage::FixedUpdate, perception::perceive)
        .add_system(Stage::FixedUpdate, perception::dispatch_perception_updates)
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
        .add_system(Stage::Startup, perception::reset_sight_rays);
}

/// Before any script draws, so a one-tick line lives until the next tick.
fn age_debug_lines(world: &mut World, _res: &mut Resources) {
    world.scene.borrow_mut().debug_lines.tick();
}

fn init_scripts(_world: &mut World, res: &mut Resources) {
    res.script_manager.init_scripts();
}

/// After this tick's UI callbacks, so gameplay reads `UI.IsPointerConsumed` settled.
fn update_scripts(_world: &mut World, res: &mut Resources) {
    res.script_manager.update_scripts(res.frame_dt);
}

fn tick_script_timers(_world: &mut World, res: &mut Resources) {
    res.script_manager.tick_timers(res.frame_dt);
}

fn serve_queries(_world: &mut World, res: &mut Resources) {
    res.script_manager.serve_queries();
}

fn late_update_scripts(_world: &mut World, res: &mut Resources) {
    res.script_manager.late_update_scripts(res.frame_dt);
}

/// Unity's end-of-frame `Object.Destroy`: a destroyed entity took part in the whole tick.
fn apply_destroys(_world: &mut World, res: &mut Resources) {
    let removed = res.script_manager.apply_pending_destroys();
    let tick = res.play_frame();
    res.audio.borrow_mut().stop_sources(removed, tick);
}

fn solve_ik(world: &mut World, _res: &mut Resources) {
    world.scene.borrow_mut().solve_ik();
}

fn advance_frame(_world: &mut World, res: &mut Resources) {
    res.play_frame += 1;
}

fn rebake_nav(world: &mut World, res: &mut Resources) {
    let mut scene = world.scene.borrow_mut();
    rusty_navigation::tick_obstacles(&mut scene, res.frame_dt);
    let mut nav = res.nav.borrow_mut();
    // Frame 0 restarts from the scene's saved navmesh or a full bake.
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

/// Root motion moves outside any script, so its controller hits wait here.
fn dispatch_animation_events(_world: &mut World, res: &mut Resources) {
    let queued = std::mem::take(&mut res.animation_events);
    res.script_manager.dispatch_state_callbacks(&queued.states);
    res.script_manager.dispatch_animation_events(&queued.events);
    let hits = std::mem::take(&mut res.controller_hits);
    res.script_manager.dispatch_queued_controller_hits(&hits);
}

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

fn save_hulls() {
    #[cfg(any(feature = "editor", feature = "dev"))]
    rusty_asset::import_cache::hulls::save_pending();
}
