mod activity;

use std::cell::RefCell;
use std::rc::Rc;

use super::registry::App;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;
use rusty_audio::Occluder;
use rusty_physics::{is_hittable, PhysicsWorld};
use rusty_scene::{game_camera_from_scene, layer_in_mask, Scene};

pub(super) fn register(app: &mut App) {
    // `Startup` follows the scripts' `Start`, which may have changed a source's flags.
    app.add_system(Stage::Startup, start_play_on_start);
    app.add_system(Stage::LateUpdate, activity::follow_activity);
    app.add_system(Stage::LateUpdate, advance_mixer);
    app.add_system(Stage::LateUpdate, occlude_voices);
    app.add_system(Stage::LateUpdate, resolve_reverb);
}

impl super::GameWorld {
    /// Run by the tick and a paused frame, never a system: no device is threaded through
    /// a deterministic stage.
    pub fn push_audio(&self, seconds: f32) {
        let res = &self.resources;
        let (scene, time) = (&self.world.scene, &res.time);
        rusty_script::api::audio::push_frame(scene, &res.audio, time, &res.camera, seconds.into());
    }
}

fn resolve_reverb(world: &mut World, res: &mut Resources) {
    let scene = world.scene.borrow();
    let listener = game_camera_from_scene(&res.camera.borrow(), &scene).position;
    let mut ids = scene.world.ids_with_reverb_zone();
    ids.retain(|&id| scene.world.is_active_in_hierarchy(id));
    ids.sort_unstable(); // `blend` sums in this order
    let zones: Vec<_> = ids
        .iter()
        .filter_map(|&id| {
            let zone = scene.world.reverb_zone(id)?.clone();
            Some((scene.world.world_position(id), zone))
        })
        .collect();
    let refs: Vec<_> = zones.iter().map(|(at, zone)| (*at, zone)).collect();
    res.audio.borrow_mut().resolve_reverb(listener, &refs);
}

fn occlude_voices(world: &mut World, res: &mut Resources) {
    let dt = res.time.borrow().unscaled_delta_time;
    let scene = world.scene.borrow();
    let listener = game_camera_from_scene(&res.camera.borrow(), &scene).position;
    let lookup = |id| {
        let source = scene.world.audio(id)?.clone();
        Some((source, scene.world.world_position(id)))
    };
    res.audio.borrow_mut().occlude(listener, dt, lookup);
}

pub(super) fn occluder(
    scene: Rc<RefCell<Scene>>,
    physics: Rc<RefCell<Option<PhysicsWorld>>>,
) -> Occluder {
    Box::new(move |from, to, ignore, mask| {
        let (Ok(scene), Ok(physics)) = (scene.try_borrow(), physics.try_borrow()) else {
            return false;
        };
        let Some(physics) = physics.as_ref() else {
            return false;
        };
        // The source and its ancestors: a voice on a character's child is not hidden by its body.
        let skip: Vec<u32> = std::iter::successors(ignore, |&id| scene.world.parent_id(id))
            .take(64)
            .collect();
        physics.segment_blocked(from, to, |id| {
            !skip.contains(&id)
                && is_hittable(&scene, id)
                && layer_in_mask(scene.world.layer(id), mask)
        })
    })
}

/// On unscaled sim time, not the wall clock, so a replay reads back the same mix.
fn advance_mixer(_world: &mut World, res: &mut Resources) {
    let now = res.time.borrow().unscaled_time;
    res.audio.borrow_mut().advance_mixer(now);
}

fn start_play_on_start(world: &mut World, res: &mut Resources) {
    start_sources(world, res, |_| true);
}

/// `admit` passes the whole scene at Play, only the incoming entities after a scene load.
pub(super) fn start_sources(world: &World, res: &Resources, admit: impl Fn(u32) -> bool) {
    type Row = (u32, rusty_components::AudioSourceComponent, [f32; 3]);
    let rows: Vec<Row> = {
        let scene = world.scene.borrow();
        scene
            .world
            .ids_with_audio()
            .into_iter()
            .filter(|&id| admit(id) && scene.world.is_active_in_hierarchy(id))
            .filter_map(|id| {
                let src = scene.world.audio(id)?;
                if !src.play_on_start {
                    return None;
                }
                let p = scene.world.world_position(id);
                Some((id, src.clone(), [p.x, p.y, p.z]))
            })
            .collect()
    };
    let tick = res.play_frame();
    let mut audio = res.audio.borrow_mut();
    for (id, src, pos) in rows {
        audio.note_source_active(id, true);
        audio.play_source(id, &src, pos, tick);
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod audio_tests;

#[cfg(test)]
#[path = "occlusion_tests.rs"]
mod audio_occlusion_tests;

#[cfg(test)]
#[path = "reverb_tests.rs"]
mod audio_reverb_tests;

#[cfg(test)]
#[path = "activity_tests.rs"]
mod audio_activity_tests;

#[cfg(test)]
#[path = "push_tests.rs"]
mod audio_push_tests;

#[cfg(test)]
#[path = "doppler_tests.rs"]
mod audio_doppler_tests;
