//! src/app/audio.rs — audio play-mode system (#212).
//!
//! One `Startup`-stage system: when the world enters Play, start every
//! `AudioSource` flagged `play_on_start` (Unity's "Play On Awake"). It drives the
//! shared `AudioMaestro` exactly as the `Audio` API does, so a `play_on_start`
//! voice and a scripted `Audio.Play` land in one device and one introspection log.
//!
//! This is a *bookkeeping* system: on the harness's `NullBackend` it produces no
//! sound but still records the play events, so a headless play-test can assert that
//! the right sources auto-started. Per-source continuous playback needs no per-frame
//! sim work — rodio loops a voice on its own thread — so there is no `FixedUpdate`
//! audio system to thread a device through the deterministic stage.

use super::registry::App;
use super::resources::Resources;
use super::stage::Stage;
use super::world::World;

/// Register the audio systems. The auto-start runs once in `Startup`, after the
/// scene + scripts are live (so a `Start` script could already have toggled a
/// source's flags before it fires).
pub(super) fn register(app: &mut App) {
    app.add_system(Stage::Startup, start_play_on_start);
}

/// Start every `play_on_start` AudioSource in the freshly-entered Play session.
fn start_play_on_start(world: &mut World, res: &mut Resources) {
    // Collect the auto-start sources first so the scene borrow doesn't overlap the
    // maestro mutation.
    type Row = (u32, crate::components::AudioSourceComponent, [f32; 3]);
    let rows: Vec<Row> = {
        let scene = world.scene.borrow();
        scene
            .world
            .ids_with_audio()
            .into_iter()
            .filter(|&id| scene.world.is_active(id))
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
