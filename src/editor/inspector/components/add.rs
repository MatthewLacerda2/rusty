use egui_phosphor::regular as icon;

use crate::scene::authoring;
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

/// Add-menu entries for rigidbody, material/texture and nav-agent. Each entry is
/// offered only when absent.
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

/// The rendering half of the Add Component menu: camera, particles and the
/// camera-only visual correction stack. Each entry is offered only when absent.
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
    if world.has_camera(id)
        && !world.has_visual_correction(id)
        && ui.button("Visual Correction Component").clicked()
    {
        world.set_visual_correction(id, Some(authoring::default_visual_correction()));
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
