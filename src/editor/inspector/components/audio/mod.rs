//! src/editor/inspector/components/audio/ — the audio cards: the AudioSource card
//! (`source.rs`, #212) and the AudioReverbZone card (`reverb_zone.rs`, #469).

mod reverb_zone;
mod source;

/// Every audio card the entity carries, in inspector order.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    source::draw(ui, world, id, is_dirty);
    reverb_zone::draw(ui, world, id, is_dirty);
}
