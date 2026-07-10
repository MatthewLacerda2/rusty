//! src/editor/inspector/components/gameplay/ — the gameplay inspector cards. This
//! `mod.rs` holds the Lua Script and Animator cards; the physics-flavoured cards
//! live apart to stay under the size cap: the Collider card in `collider`, the
//! RigidBody and NavMesh Agent cards in `physics`.

use egui_phosphor::regular as icon;
use std::path::Path;

mod collider;
mod physics;

pub use collider::draw_collider;
pub use physics::{draw_nav_agent, draw_rigidbody};

use crate::editor::inspector::components::card::component_card;
use crate::editor::theme;
use crate::scene::authoring::animator as animator_ops;

/// 3D. Script bindings — one card per attached script (#83). An entity can carry
/// many scripts; each renders as its own removable Lua Script card.
pub fn draw_script(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    // Collect the index to remove (if any) so we don't mutate the vec mid-draw.
    let mut remove_index: Option<usize> = None;
    let Some(mut scripts) = world.scripts_mut(id) else {
        return;
    };
    for (i, script) in scripts.iter_mut().enumerate() {
        let mut remove = false;
        component_card(ui, icon::SCROLL, "Lua Script", Some(&mut remove), |ui| {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut script.path);
            });
            let t = theme::from_ui(ui);
            if Path::new(&script.path).exists() {
                ui.colored_label(t.accent_blue, "✔ Loaded and ready to run");
            } else {
                ui.colored_label(t.danger, "❌ File not found!");
            }
            // Schema-driven field controls (#84): one typed control per field the
            // script's optional `fields` table declares. No-op for plain scripts.
            crate::editor::inspector::components::script_fields::draw_script_fields(
                ui, script, is_dirty,
            );
        });
        if remove {
            remove_index = Some(i);
        }
    }
    if let Some(i) = remove_index {
        scripts.remove(i);
        *is_dirty = true;
    }
}

/// 3E. Animator Component — the entity's skeletal-animation playback state. The
/// clips themselves live on the skinned mesh; this card edits which one plays and
/// how.
pub fn draw_animator(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
) {
    let Some(anim) = world.animator(id).map(|a| a.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(
        ui,
        icon::PERSON_SIMPLE_RUN,
        "Animator",
        Some(&mut remove),
        |ui| animator_card_body(ui, world, id, &anim, is_dirty),
    );
    if remove {
        world.set_animator(id, None);
        *is_dirty = true;
    }
}

/// The Animator card's widgets: every field write routes through the shared
/// `authoring::animator` ops (#287), the same writes the `Animator.*` Lua
/// bindings use.
fn animator_card_body(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    anim: &crate::components::AnimatorComponent,
    is_dirty: &mut bool,
) {
    let mut clip = anim.current_clip.clone();
    ui.horizontal(|ui| {
        ui.label("Clip:");
        if ui.text_edit_singleline(&mut clip).changed() {
            if let Some(mut a) = world.animator_mut(id) {
                animator_ops::set_clip(&mut a, clip);
            }
            *is_dirty = true;
        }
    });
    let mut speed = anim.speed;
    ui.horizontal(|ui| {
        ui.label("Speed:");
        if ui
            .add(egui::DragValue::new(&mut speed).speed(0.05))
            .changed()
        {
            if let Some(mut a) = world.animator_mut(id) {
                animator_ops::set_speed(&mut a, speed);
            }
            *is_dirty = true;
        }
    });
    let mut is_playing = anim.is_playing;
    if ui.checkbox(&mut is_playing, "Is Playing").changed() {
        if let Some(mut a) = world.animator_mut(id) {
            animator_ops::set_playing(&mut a, is_playing);
        }
        *is_dirty = true;
    }
    let mut freeze = anim.freeze;
    if ui.checkbox(&mut freeze, "Freeze").changed() {
        if let Some(mut a) = world.animator_mut(id) {
            animator_ops::set_freeze(&mut a, freeze);
        }
        *is_dirty = true;
    }
    let mut loop_clip = anim.loop_clip;
    if ui.checkbox(&mut loop_clip, "Loop").changed() {
        if let Some(mut a) = world.animator_mut(id) {
            animator_ops::set_looping(&mut a, loop_clip);
        }
        *is_dirty = true;
    }
    draw_graph(ui, world, id, anim, is_dirty);
    draw_parameters(ui, anim);
}

/// The animation-graph reference (#315/#316): the `.animgraph` path driving this
/// animator, the auto-evaluation toggle, and (read-only) the active graph state.
/// Writes route through the shared `authoring::animator` ops, like every other
/// field on the card.
fn draw_graph(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    anim: &crate::components::AnimatorComponent,
    is_dirty: &mut bool,
) {
    ui.separator();
    let mut graph = anim.graph.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Graph:");
        if ui.text_edit_singleline(&mut graph).changed() {
            if let Some(mut a) = world.animator_mut(id) {
                animator_ops::set_graph(&mut a, (!graph.is_empty()).then_some(graph));
            }
            *is_dirty = true;
        }
    });
    if anim.graph.is_none() {
        return;
    }
    let mut enabled = anim.graph_enabled;
    if ui.checkbox(&mut enabled, "Graph enabled").changed() {
        if let Some(mut a) = world.animator_mut(id) {
            animator_ops::set_graph_enabled(&mut a, enabled);
        }
        *is_dirty = true;
    }
    if let Some(node) = &anim.current_node {
        ui.label(format!("State: {node}"));
    }
}

/// Read-only listing of the animator's typed graph parameters (#314). They are
/// runtime state driven by `Animator.Set*` (and, later, authored defaults on the
/// graph asset, #315), so the card shows them without offering edits.
fn draw_parameters(ui: &mut egui::Ui, anim: &crate::components::AnimatorComponent) {
    if anim.parameters.is_empty() {
        return;
    }
    ui.separator();
    ui.label("Parameters (set via Animator.Set*):");
    for (name, value) in &anim.parameters {
        ui.label(format!("    {name}: {value}"));
    }
}
