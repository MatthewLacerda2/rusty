//! src/app/audio/activity.rs — an `AudioSource` follows its entity's activity (#1363).
//!
//! Unity's `AudioSource.OnDisable` stops its voice and its play-on-awake plays again
//! on enable. Each `LateUpdate` compares every source's `activeInHierarchy` with the
//! last frame's (`AudioMaestro::note_source_active`): the falling edge silences the
//! voice, the rising edge restarts a `play_on_start` source. Hiding a parent pivot
//! is enough; the children's own flags are never read alone.

use super::super::resources::Resources;
use super::super::world::World;

/// Stop the voices of sources that went inactive in the hierarchy this frame and
/// restart the `play_on_start` ones that came back, in entity-id order. Only an edge
/// reads a position or a source; a steady frame is one kept answer per source (#1388).
pub(super) fn follow_activity(world: &mut World, res: &mut Resources) {
    let scene = world.scene.borrow();
    let mut ids = scene.world.ids_with_audio();
    ids.sort_unstable();
    let tick = res.play_frame();
    let mut audio = res.audio.borrow_mut();
    audio.retain_sources(|id| ids.binary_search(&id).is_ok());
    for id in ids {
        let active = scene.world.is_active_in_hierarchy(id);
        if !audio.note_source_active(id, active) {
            continue;
        }
        let at = scene.world.world_position(id).into();
        if !active {
            audio.stop_sources([(id, at)], tick);
        } else if let Some(source) = scene.world.audio(id).filter(|s| s.play_on_start) {
            audio.play_source(id, &source, at, tick);
        }
    }
}
