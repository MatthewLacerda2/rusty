mod reverb_zone;
mod source;

/// Every audio card the entity carries, in inspector order.
pub fn draw(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32) {
    source::draw(ui, world, id);
    reverb_zone::draw(ui, world, id);
}
