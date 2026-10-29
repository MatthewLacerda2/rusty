//! The Mesh card (Unity's *Mesh Filter* and *Mesh Renderer* together): the mesh's
//! source, a `Plane`'s subdivisions (#1524), how it casts shadows (#1571), and a
//! skinned mesh's hitbox and ragdoll tools.

use egui_phosphor::regular as icon;

use super::card::component_card;
use super::particles::widgets::combo;
use crate::components::mesh::primitives::MAX_PLANE_SUBDIVISIONS;
use crate::components::ShadowCasting;
use crate::editor::widgets::DragRange;
use crate::scene::authoring::rendering::mesh as mesh_ops;

/// The skinned-mesh tools a Mesh card button asks for, deferred because they need
/// the Scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkinTool {
    /// *Generate Hitboxes* (#464).
    Hitboxes,
    /// *Build Ragdoll* (#466).
    Ragdoll,
}

/// 3B. Mesh details. A skinned mesh also offers *Generate Hitboxes* (#464) and
/// *Build Ragdoll* (#466): the click is deferred into `skin_tool`, since both
/// need the Scene.
pub fn draw_mesh(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
    skin_tool: &mut Option<SkinTool>,
) {
    if !world.has_mesh(id) {
        return;
    }
    let mut remove = false;
    let mut subdivide = None;
    let mut casting = None;
    component_card(ui, icon::CUBE, "Mesh Filter", Some(&mut remove), |ui| {
        if let Some(mesh) = world.mesh(id) {
            ui.label(format!("Type: {} primitive", mesh.primitive_type));
            if mesh.primitive_type == "Plane" {
                subdivide = draw_subdivisions(ui, mesh.subdivisions);
            }
            let mut mode = mesh.cast_shadows;
            let modes = ShadowCasting::ALL.map(|m| (m, m.name()));
            if combo(ui, "Cast Shadows", &mut mode, &modes) {
                casting = Some(mode);
            }
            ui.label(format!(
                "Geometry: {} verts | {} indices",
                mesh.vertices.len(),
                mesh.indices.len()
            ));
            if mesh.skin.is_some() {
                ui.horizontal(|ui| {
                    let hitboxes = ui
                        .button("Generate Hitboxes")
                        .on_hover_text("One collider per bone, on the Hitbox layer");
                    if hitboxes.clicked() {
                        *skin_tool = Some(SkinTool::Hitboxes);
                    }
                    let ragdoll = ui
                        .button("Build Ragdoll")
                        .on_hover_text("A Rigidbody and Joint on each hitbox bone");
                    if ragdoll.clicked() {
                        *skin_tool = Some(SkinTool::Ragdoll);
                    }
                });
            }
        }
    });
    if let (Some(n), Some(mut mesh)) = (subdivide, world.mesh_mut(id)) {
        // The drag holds the range, so the op has nothing to refuse.
        let _ = mesh_ops::set_subdivisions(&mut mesh, n);
        *is_dirty = true;
    }
    if let (Some(mode), Some(mut mesh)) = (casting, world.mesh_mut(id)) {
        mesh_ops::set_cast_shadows(&mut mesh, mode);
        *is_dirty = true;
    }
    if remove {
        world.set_mesh(id, None);
        *is_dirty = true;
    }
}

/// A `Plane`'s quads per side (#1524): the new count once an edit finishes (a
/// released drag or a typed value). Mid-drag the count is held in egui's memory, so
/// the grid is rebuilt and re-uploaded once, not every frame of the drag.
fn draw_subdivisions(ui: &mut egui::Ui, current: u32) -> Option<u32> {
    let held = ui.id().with("mesh_subdivisions");
    let mut n = ui.data(|d| d.get_temp::<u32>(held)).unwrap_or(current);
    let drag = egui::DragValue::new(&mut n).ranged(1..=MAX_PLANE_SUBDIVISIONS);
    let r = ui
        .horizontal(|ui| {
            ui.label("Subdivisions:")
                .on_hover_text("Quads per side: vertices for a vertex shader to move");
            ui.add(drag)
        })
        .inner;
    if r.dragged() && !r.drag_stopped() {
        ui.data_mut(|d| d.insert_temp(held, n));
        return None;
    }
    ui.data_mut(|d| d.remove::<u32>(held));
    let finished = r.drag_stopped() || r.changed();
    (finished && n != current).then_some(n)
}
