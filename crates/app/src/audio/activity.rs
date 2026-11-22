use super::super::resources::Resources;
use super::super::world::World;

/// Unity's: a source going inactive in the hierarchy stops its voice, and a play-on-awake
/// one plays again on coming back.
pub(super) fn follow_activity(world: &mut World, res: &mut Resources) {
    let scene = world.scene.borrow();
    let mut ids = scene.world.ids_with_audio();
    ids.sort_unstable(); // id order, and `binary_search` below needs it
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
