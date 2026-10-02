//! src/app/audio.rs — audio play-mode system (#212).
//!
//! A `Startup`-stage system: when the world enters Play, start every
//! `AudioSource` flagged `play_on_start` (Unity's "Play On Awake"). And a
//! `LateUpdate` one stepping the mixer (#465), voice occlusion (#467) and the
//! listener's reverb zones (#469) — snapshot blends and ducking — on
//! `Time.unscaledTime`, so the mix state is a function of sim time and a replay
//! reads back the same groups. It drives the
//! shared `AudioMaestro` exactly as the `Audio` API does, so a `play_on_start`
//! voice and a scripted `Audio.Play` land in one device and one introspection log.
//!
//! This is a *bookkeeping* system: on the harness's `NullBackend` it produces no
//! sound but still records the play events, so a headless play-test can assert that
//! the right sources auto-started. The per-frame mix (3D rolloff, pan, time scale)
//! is not a sim system: the windowed shell applies it after each advance
//! (`shell/audio.rs`, #412), so no device is threaded through a deterministic stage.

use std::cell::RefCell;
use std::rc::Rc;

use super::registry::App;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;
use crate::audio::Occluder;
use crate::physics::{is_hittable, PhysicsWorld};
use crate::scene::{layer_in_mask, Scene};

/// Register the audio systems. The auto-start runs once in `Startup`, after the
/// scene + scripts are live (so a `Start` script could already have toggled a
/// source's flags before it fires).
pub(super) fn register(app: &mut App) {
    app.add_system(Stage::Startup, start_play_on_start);
    app.add_system(Stage::LateUpdate, advance_mixer);
    app.add_system(Stage::LateUpdate, occlude_voices);
    app.add_system(Stage::LateUpdate, resolve_reverb);
}

/// Blend the reverb zones around the active camera onto the reverb bus (#469).
/// Zones are summed in entity-id order, so two runs blend bit-identically.
fn resolve_reverb(world: &mut World, res: &mut Resources) {
    let listener = res.camera.borrow().position;
    let scene = world.scene.borrow();
    let mut ids = scene.world.ids_with_reverb_zone();
    ids.retain(|&id| scene.world.is_active(id));
    ids.sort_unstable();
    let zones: Vec<_> = ids
        .iter()
        .filter_map(|&id| {
            let zone = scene.world.reverb_zone(id)?.clone();
            Some((scene.world_matrix(id).w_axis.truncate(), zone))
        })
        .collect();
    let refs: Vec<_> = zones.iter().map(|(at, zone)| (*at, zone)).collect();
    res.audio.borrow_mut().resolve_reverb(listener, &refs);
}

/// Re-cast and smooth every voice's occlusion (#467) against the active camera,
/// on unscaled time like the mixer.
fn occlude_voices(world: &mut World, res: &mut Resources) {
    let dt = res.time.borrow().unscaled_delta_time;
    let listener = res.camera.borrow().position;
    let scene = world.scene.borrow();
    let lookup = |id| {
        let source = scene.world.audio(id)?.clone();
        Some((source, scene.world.transform(id)?.position))
    };
    res.audio.borrow_mut().occlude(listener, dt, lookup);
}

/// The physics query audio occlusion casts through: a solid collider on an
/// occluding layer between the two points, looking past the source's own entity
/// and its ancestors (a voice on a character's child is not hidden by its body).
/// Reads nothing while either cell is busy or no physics world is built.
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

/// Step the mixer to this frame's unscaled sim time.
fn advance_mixer(_world: &mut World, res: &mut Resources) {
    let now = res.time.borrow().unscaled_time;
    res.audio.borrow_mut().advance_mixer(now);
}

/// Start every `play_on_start` AudioSource in the freshly-entered Play session.
fn start_play_on_start(world: &mut World, res: &mut Resources) {
    start_sources(world, res, |_| true);
}

/// Start every active `play_on_start` AudioSource whose entity passes `admit` — the
/// whole scene at Play, only the incoming entities after a scene load (#432).
pub(super) fn start_sources(world: &World, res: &Resources, admit: impl Fn(u32) -> bool) {
    // Collect the auto-start sources first so the scene borrow doesn't overlap the
    // maestro mutation.
    type Row = (u32, crate::components::AudioSourceComponent, [f32; 3]);
    let rows: Vec<Row> = {
        let scene = world.scene.borrow();
        scene
            .world
            .ids_with_audio()
            .into_iter()
            .filter(|&id| admit(id) && scene.world.is_active(id))
            .filter_map(|id| {
                let src = scene.world.audio(id)?;
                if !src.play_on_start {
                    return None;
                }
                let p = scene.world.transform(id)?.position;
                Some((id, src.clone(), [p.x, p.y, p.z]))
            })
            .collect()
    };
    let tick = res.play_frame();
    let mut audio = res.audio.borrow_mut();
    for (id, src, pos) in rows {
        audio.play_source(id, &src, pos, tick);
    }
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod audio_tests;

#[cfg(test)]
#[path = "audio_occlusion_tests.rs"]
mod audio_occlusion_tests;
