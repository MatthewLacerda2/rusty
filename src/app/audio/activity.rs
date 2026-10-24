//! src/app/audio/activity.rs — an `AudioSource` follows its entity's activity (#1363).
//!
//! Unity's `AudioSource.OnDisable` stops its voice and its play-on-awake plays again
//! on enable. Each `LateUpdate` compares every source's `activeInHierarchy` with the
//! last frame's (`AudioMaestro::note_source_active`): the falling edge silences the
//! voice, the rising edge restarts a `play_on_start` source. Hiding a parent pivot
//! is enough; the children's own flags are never read alone.

use super::super::resources::Resources;
use super::super::world::World;
use crate::components::AudioSourceComponent;

/// Stop the voices of sources that went inactive in the hierarchy this frame and
/// restart the `play_on_start` ones that came back, in entity-id order.
pub(super) fn follow_activity(world: &mut World, res: &mut Resources) {
    type Row = (u32, bool, AudioSourceComponent, [f32; 3]);
    let rows: Vec<Row> = {
        let scene = world.scene.borrow();
        let mut ids = scene.world.ids_with_audio();
        ids.sort_unstable();
        ids.into_iter()
            .filter_map(|id| {
                let source = scene.world.audio(id)?.clone();
                let at = scene.world.world_position(id);
                Some((
                    id,
                    scene.world.is_active_in_hierarchy(id),
                    source,
                    at.into(),
                ))
            })
            .collect()
    };
    let tick = res.play_frame();
    let mut audio = res.audio.borrow_mut();
    audio.retain_sources(|id| rows.iter().any(|row| row.0 == id));
    for (id, active, source, at) in rows {
        if !audio.note_source_active(id, active) {
            continue;
        }
        if !active {
            audio.stop_sources([(id, at)], tick);
        } else if source.play_on_start {
            audio.play_source(id, &source, at, tick);
        }
    }
}
