//! src/editor/inspector/components/gameplay/ — the gameplay inspector cards. This
//! `mod.rs` holds the Lua Script and Animator cards; the physics-flavoured cards
//! (Collider, RigidBody, NavMesh Agent) live in `physics`, split apart to stay under
//! the size cap.

use egui_phosphor::regular as icon;
use std::path::Path;

mod physics;

pub use physics::{draw_collider, draw_nav_agent, draw_rigidbody};

use crate::editor::inspector::components::card::component_card;
use crate::editor::theme;
use crate::scene::authoring::animator as animator_ops;
use crate::scene::Entity;

/// 3D. Script bindings — one card per attached script (#83). An entity can carry
/// many scripts; each renders as its own removable Lua Script card.
pub fn draw_script(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    // Collect the index to remove (if any) so we don't mutate the vec mid-draw.
    let mut remove_index: Option<usize> = None;
    for (i, script) in entity.scripts.iter_mut().enumerate() {
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
        entity.scripts.remove(i);
        *is_dirty = true;
    }
}

/// 3E. Animator Component — the entity's skeletal-animation playback state. The
/// clips themselves live on the skinned mesh; this card edits which one plays and
/// how.
pub fn draw_animator(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    if entity.animator.is_none() {
        return;
    }
    let mut remove = false;
    let Some(anim) = entity.animator.clone() else {
        return;
    };
    component_card(
        ui,
        icon::PERSON_SIMPLE_RUN,
        "Animator",
        Some(&mut remove),
        |ui| {
            let mut clip = anim.current_clip.clone();
            ui.horizontal(|ui| {
                ui.label("Clip:");
                if ui.text_edit_singleline(&mut clip).changed() {
                    animator_ops::set_clip(entity, clip);
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
                    animator_ops::set_speed(entity, speed);
                    *is_dirty = true;
                }
            });
            let mut is_playing = anim.is_playing;
            if ui.checkbox(&mut is_playing, "Is Playing").changed() {
                animator_ops::set_playing(entity, is_playing);
                *is_dirty = true;
            }
            let mut freeze = anim.freeze;
            if ui.checkbox(&mut freeze, "Freeze").changed() {
                animator_ops::set_freeze(entity, freeze);
                *is_dirty = true;
            }
        },
    );
    if remove {
        entity.animator = None;
        *is_dirty = true;
    }
}
