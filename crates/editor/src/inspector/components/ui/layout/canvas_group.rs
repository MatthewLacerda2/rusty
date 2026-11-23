//! The Canvas Group inspector card: subtree alpha plus the interactable and
//! blocks-raycasts flags. Every write routes through `edit::authoring::ui::layout::canvas_group`.

use crate::inspector::components::card::remove_component;
use egui_phosphor::regular as icon;
use rusty_edit::authoring::ComponentKind;

use crate::inspector::components::card::component_card;
use rusty_edit::authoring::ui::layout::canvas_group as group_ops;

/// Canvas Group card. A THIN client: widgets read a snapshot and route each
/// write through a shared op; remove detaches the component.
pub fn draw(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32) {
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
                ui.add(crate::widgets::slider(&mut alpha, 0.0..=1.0))
                    .changed()
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
        remove_component(world, id, ComponentKind::CanvasGroup);
    }
}

impl crate::inspector::components::add::AddEntry for rusty_components::CanvasGroupComponent {
    const CATEGORY: crate::inspector::components::add::Category =
        crate::inspector::components::add::Category::Ui;
    const LABEL: &'static str = "Canvas Group";
    const ICON: &'static str = egui_phosphor::regular::STACK;
}
