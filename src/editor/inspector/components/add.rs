use egui_phosphor::regular as icon;

use crate::components::{CanvasComponent, CanvasGroupComponent, RectTransformComponent};
use crate::scene::authoring::{self, ComponentKind};
use crate::scene::{
    AudioSourceComponent, MaterialComponent, ParticleEmitterComponent, ScriptComponent,
    DEFAULT_SCRIPTS_DEST_DIR,
};

/// 3F. Add Component — Unity-style full-width pill that opens the component menu.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    ui.add_space(crate::editor::theme::from_ui(ui).space_sm);
    // A justified layout stretches the menu button to the full panel width.
    ui.with_layout(
        egui::Layout::top_down_justified(egui::Align::Center),
        |ui| {
            ui.menu_button(format!("{}  Add Component", icon::PLUS), |ui| {
                add_menu(ui, world, id);
            });
        },
    );
}

fn add_menu(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    // Every project `.lua` that exposes a lifecycle table is a MonoBehaviour and
    // is offered here with the moon glyph (the per-type icons for the other
    // built-ins are the burn-down in #82); helper modules without a lifecycle
    // table are not. An entity can hold many scripts (#83), so each pick appends.
    add_script_menu(ui, world, id);
    add_lighting_combat(ui, world, id);
    add_physics_components(ui, world, id);
    add_render_components(ui, world, id);
    add_audio(ui, world, id);
    add_ui_components(ui, world, id);
    add_layout_components(ui, world, id);
}

/// Add-menu entries for the in-game UI (#417, #418, #419, #420): a Canvas makes the entity
/// a UI root; a RectTransform makes it a UI element laid out inside its parent's
/// rect; an Image draws it; a Text labels it; a Canvas Group fades its subtree; a
/// Rect Mask clips it; a Selectable makes it interactive (#420). Each is offered only
/// when absent. Image, Text, Rect Mask and Selectable declare `requires(RectTransform)`, so they go through the shared dependency
/// verb, matching `Scene.AddComponent`.
fn add_ui_components(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    if !world.has_canvas(id) && ui.button(format!("{}  Canvas", icon::MONITOR)).clicked() {
        world.set_canvas(id, Some(CanvasComponent::default()));
        ui.close_menu();
    }
    if !world.has_rect_transform(id)
        && ui
            .button(format!("{}  Rect Transform", icon::FRAME_CORNERS))
            .clicked()
    {
        world.set_rect_transform(id, Some(RectTransformComponent::default()));
        ui.close_menu();
    }
    if !world.has_image(id) && ui.button(format!("{}  Image", icon::IMAGE)).clicked() {
        authoring::add_with_requirements(world, id, ComponentKind::Image);
        ui.close_menu();
    }
    if !world.has_canvas_group(id)
        && ui
            .button(format!("{}  Canvas Group", icon::STACK))
            .clicked()
    {
        world.set_canvas_group(id, Some(CanvasGroupComponent::default()));
        ui.close_menu();
    }
    if !world.has_text(id) && ui.button(format!("{}  Text", icon::TEXT_T)).clicked() {
        authoring::add_with_requirements(world, id, ComponentKind::Text);
        ui.close_menu();
    }
    if !world.has_rect_mask(id) && ui.button(format!("{}  Rect Mask", icon::CROP)).clicked() {
        authoring::add_with_requirements(world, id, ComponentKind::RectMask);
        ui.close_menu();
    }
    if !world.has_selectable(id)
        && ui
            .button(format!("{}  Selectable", icon::CURSOR_CLICK))
            .clicked()
    {
        authoring::add_with_requirements(world, id, ComponentKind::Selectable);
        ui.close_menu();
    }
}

/// Add-menu entries for UI auto-layout (#421): a Layout Group arranges the children
/// in a row, column or grid; a Layout Element overrides an element's layout sizes
/// and fits it to its content. Both require a RectTransform, via the shared
/// dependency verb.
fn add_layout_components(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    if !world.has_layout_group(id)
        && ui
            .button(format!("{}  Layout Group", icon::LAYOUT))
            .clicked()
    {
        authoring::add_with_requirements(world, id, ComponentKind::LayoutGroup);
        ui.close_menu();
    }
    if !world.has_layout_element(id)
        && ui
            .button(format!("{}  Layout Element", icon::ARROWS_OUT))
            .clicked()
    {
        authoring::add_with_requirements(world, id, ComponentKind::LayoutElement);
        ui.close_menu();
    }
}

/// Add-menu entry for the AudioSource component (#212). Offered only when absent.
fn add_audio(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    if !world.has_audio(id)
        && ui
            .button(format!("{}  Audio Source", icon::SPEAKER_HIGH))
            .clicked()
    {
        world.set_audio(id, Some(AudioSourceComponent::default()));
        ui.close_menu();
    }
}

/// Add-menu entries for light, animator and collider. Each entry is offered only
/// when absent. Animator has its own entry (#82) and is added/removed
/// independently.
fn add_lighting_combat(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    if !world.has_light(id) && ui.button("Light Component").clicked() {
        world.set_light(id, Some(authoring::default_light()));
        ui.close_menu();
    }
    if !world.has_animator(id) && ui.button("Animator Component").clicked() {
        world.set_animator(id, Some(authoring::default_animator()));
        ui.close_menu();
    }
    if !world.has_collider(id) && ui.button("Collider Component").clicked() {
        world.set_collider(id, Some(authoring::default_collider()));
        ui.close_menu();
    }
}

/// Add-menu entries for rigidbody, material/texture, nav-agent and joint. Each entry
/// is offered only when absent. A Joint declares `requires(RigidBody)` (#449), so it
/// goes through the shared dependency verb, matching `Scene.AddComponent`.
fn add_physics_components(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    if !world.has_rigidbody(id) && ui.button("RigidBody Component").clicked() {
        world.set_rigidbody(id, Some(authoring::default_rigidbody()));
        ui.close_menu();
    }
    if !world.has_material(id) && ui.button("Material / Texture Component").clicked() {
        attach_default_material(world, id);
        ui.close_menu();
    }
    if !world.has_nav_agent(id) && ui.button("NavMesh Agent Component").clicked() {
        world.set_nav_agent(id, Some(authoring::default_nav_agent()));
        ui.close_menu();
    }
    if !world.has_joint(id) && ui.button(format!("{}  Joint", icon::LINK)).clicked() {
        authoring::add_with_requirements(world, id, ComponentKind::Joint);
        ui.close_menu();
    }
}

/// Attach a `MaterialComponent` referencing a fresh per-entity library material.
/// The Add menu has only the entity (not the scene), so the default `MaterialAsset`
/// rides along as the entity's staged pending material; the inspector folds it into
/// `scene.materials` once the entity guard drops (where the library is reachable).
fn attach_default_material(world: &mut crate::ecs::World, id: u32) {
    let key = format!("entity_{id}_material");
    world.set_material(id, Some(MaterialComponent { material: key }));
    world.stage_pending_material(id, authoring::default_material());
}

/// The rendering half of the Add Component menu: camera, particles and the visual
/// correction stack. Each entry is offered only when absent. `VisualCorrection`
/// declares `requires(Camera)` (#359), so picking it auto-adds a `Camera` if missing
/// through the shared dependency verb — the menu no longer gates the entry on a camera
/// being present, matching the `Scene.AddComponent` API exactly.
fn add_render_components(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    if !world.has_camera(id) && ui.button("Camera Component").clicked() {
        world.set_camera(id, Some(authoring::default_camera()));
        ui.close_menu();
    }
    if !world.has_particles(id)
        && ui
            .button(format!("{}  Particle System", icon::SPARKLE))
            .clicked()
    {
        world.set_particles(id, Some(ParticleEmitterComponent::default()));
        ui.close_menu();
    }
    if !world.has_visual_correction(id) && ui.button("Visual Correction Component").clicked() {
        authoring::add_with_requirements(world, id, ComponentKind::VisualCorrection);
        ui.close_menu();
    }
    if !world.has_lod_group(id)
        && ui
            .button(format!("{}  LOD Group", icon::STACK_SIMPLE))
            .clicked()
    {
        authoring::add_with_requirements(world, id, ComponentKind::LodGroup);
        ui.close_menu();
    }
}

/// List the project's MonoBehaviour scripts (any `.lua` exposing a lifecycle
/// table) under the moon glyph; picking one appends a `ScriptComponent` to the
/// entity's `scripts` (#83). Duplicates are allowed, mirroring Unity. When the
/// project has no such scripts, a disabled hint is shown instead.
fn add_script_menu(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32) {
    let scripts = crate::scripting::monobehaviour_scripts(DEFAULT_SCRIPTS_DEST_DIR);
    if scripts.is_empty() {
        ui.add_enabled(
            false,
            egui::Button::new(format!("{}  No script behaviours found", icon::MOON)),
        );
        return;
    }
    for path in scripts {
        let label = crate::scripting::script_label(&path);
        if ui.button(format!("{}  {}", icon::MOON, label)).clicked() {
            if let Some(mut scripts) = world.scripts_mut(id) {
                scripts.push(ScriptComponent {
                    path,
                    is_loaded: false,
                    ..Default::default()
                });
            }
            ui.close_menu();
        }
    }
}
