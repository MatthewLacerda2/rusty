//! The Mesh card (Unity's *Mesh Filter* and *Mesh Renderer* together): the mesh's
//! source, a `Plane`'s subdivisions (#1524), how it casts shadows (#1571), its
//! Scale In Lightmap (#1704), when a camera culls it as too far or too small (#1714),
//! and a skinned mesh's hitbox and ragdoll tools.

use egui_phosphor::regular as icon;

use super::card::{component_card, remove_component};
use super::particles::widgets::combo;
use crate::widgets::DragRange;
use rusty_components::mesh::primitives::MAX_PLANE_SUBDIVISIONS;
use rusty_components::ShadowCasting;
use rusty_scene::authoring::rendering::mesh as mesh_ops;
use rusty_scene::authoring::ComponentKind;

/// The skinned-mesh tools a Mesh card button asks for, deferred because they need
/// the Scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkinTool {
    /// *Generate Hitboxes* (#464).
    Hitboxes,
    /// *Build Ragdoll* (#466).
    Ragdoll,
}

/// What the card names as the mesh's source: a scene mesh by its library name
/// (#2059), anything else by its type.
fn source_label(mesh: &rusty_components::MeshComponent) -> String {
    match rusty_scene::mesh_library::scene_mesh_name(mesh) {
        Some(name) => format!("Scene mesh: {name}"),
        None => format!("Type: {} primitive", mesh.primitive_type),
    }
}

/// 3B. Mesh details. A skinned mesh also offers *Generate Hitboxes* (#464) and
/// *Build Ragdoll* (#466): the click is deferred into `skin_tool`, since both
/// need the Scene.
pub fn draw_mesh(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
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
    let mut culling = None;
    let mut lightmap_scale = None;
    component_card(ui, icon::CUBE, "Mesh Filter", Some(&mut remove), |ui| {
        if let Some(mesh) = world.mesh(id) {
            ui.label(source_label(&mesh));
            if mesh.primitive_type == "Plane" {
                subdivide = draw_subdivisions(ui, mesh.subdivisions);
            }
            let mut mode = mesh.cast_shadows;
            let modes = ShadowCasting::ALL.map(|m| (m, m.name()));
            if combo(ui, "Cast Shadows", &mut mode, &modes) {
                casting = Some(mode);
            }
            culling = draw_culling(ui, mesh.max_draw_distance, mesh.min_screen_size);
            lightmap_scale = draw_scale_in_lightmap(ui, mesh.scale_in_lightmap);
            ui.label(format!(
                "Geometry: {} verts | {} indices",
                mesh.vertices.len(),
                mesh.indices.len()
            ));
            if mesh.skin.is_some() {
                draw_skin_tools(ui, skin_tool);
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
    if let (Some((distance, size)), Some(mut mesh)) = (culling, world.mesh_mut(id)) {
        mesh_ops::set_max_draw_distance(&mut mesh, distance);
        mesh_ops::set_min_screen_size(&mut mesh, size);
    }
    if let (Some(scale), Some(mut mesh)) = (lightmap_scale, world.mesh_mut(id)) {
        mesh_ops::set_scale_in_lightmap(&mut mesh, scale);
        *is_dirty = true;
    }
    if remove {
        // The shared verb behind `Scene.RemoveComponent`: the Material goes too.
        remove_component(world, id, ComponentKind::Mesh, is_dirty);
    }
}

/// The small-object cull (#1714): *Max Draw Distance* (metres, 0 unlimited) and *Min
/// Screen Size* (screen-relative height, 0 off). Both values when either changed.
/// Each drags at its own speed: a metre a point, and a thousandth of the screen,
/// since useful sizes sit around 0.01–0.02.
fn draw_culling(ui: &mut egui::Ui, distance: f32, size: f32) -> Option<(f32, f32)> {
    let (mut d, mut s) = (distance, size);
    let changed = drag_row(ui, "Max Draw Distance", &mut d, 1.0, 0.0..=f32::MAX)
        | drag_row(ui, "Min Screen Size", &mut s, 0.001, 0.0..=1.0);
    changed.then_some((d, s))
}

/// A labelled `f32` drag row held to `range`, moving `speed` per point dragged.
fn drag_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    speed: f64,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(speed).ranged(range))
            .changed()
    })
    .inner
}

/// Unity's *Scale In Lightmap* (#1704): the new value when it changed.
fn draw_scale_in_lightmap(ui: &mut egui::Ui, current: f32) -> Option<f32> {
    let mut scale = current;
    let range = rusty_components::MeshComponent::SCALE_IN_LIGHTMAP;
    let drag = egui::DragValue::new(&mut scale).ranged(range).speed(0.05);
    let changed = ui
        .horizontal(|ui| {
            ui.label("Scale In Lightmap:").on_hover_text(
                "Texel density multiplier for the bake; 0 leaves the mesh to light probes",
            );
            ui.add(drag).changed()
        })
        .inner;
    (changed && scale != current).then_some(scale)
}

/// *Generate Hitboxes* and *Build Ragdoll*, deferred into `skin_tool`.
fn draw_skin_tools(ui: &mut egui::Ui, skin_tool: &mut Option<SkinTool>) {
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

impl crate::inspector::components::add::AddEntry for rusty_components::MeshComponent {
    const CATEGORY: crate::inspector::components::add::Category =
        crate::inspector::components::add::Category::Rendering;
    const LABEL: &'static str = "Mesh";
    const ICON: &'static str = egui_phosphor::regular::CUBE;
}
