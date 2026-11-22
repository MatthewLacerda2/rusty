mod reverb_zone;
mod source;

/// Every audio card the entity carries, in inspector order.
pub fn draw(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, is_dirty: &mut bool) {
    source::draw(ui, world, id, is_dirty);
    reverb_zone::draw(ui, world, id, is_dirty);
}
