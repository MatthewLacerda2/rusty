//! The Canvas Group inspector card (#418): subtree alpha plus the interactable and
//! blocks-raycasts flags. Every write routes through `scene::authoring::canvas_group`.

use egui_phosphor::regular as icon;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::canvas_group as group_ops;

/// Canvas Group card. A THIN client (#287): widgets read a snapshot and route each
/// write through a shared op; remove detaches the component.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(g) = world.canvas_group(id).map(|g| g.clone()) else {
        return;
    };
    let (mut alpha, mut interactable, mut blocks) = (g.alpha, g.interactable, g.blocks_raycasts);
    let mut remove = false;
    let mut changed = false;
    component_card(ui, icon::STACK, "Canvas Group", Some(&mut remove), |ui| {
        let a = ui
            .horizontal(|ui| {
                ui.label("Alpha:");
                ui.add(egui::Slider::new(&mut alpha, 0.0..=1.0)).changed()
            })
            .inner;
        let i = ui.checkbox(&mut interactable, "Interactable").changed();
        let b = ui.checkbox(&mut blocks, "Blocks Raycasts").changed();
        if let (true, Some(mut g)) = (a || i || b, world.canvas_group_mut(id)) {
            group_ops::set_alpha(&mut g, alpha);
            group_ops::set_interactable(&mut g, interactable);
            group_ops::set_blocks_raycasts(&mut g, blocks);
            changed = true;
        }
    });
    if remove {
        world.set_canvas_group(id, None);
    }
    *is_dirty |= remove || changed;
}
