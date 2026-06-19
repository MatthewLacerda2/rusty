use egui_phosphor::regular as icon;

use crate::scene::authoring;
use crate::scene::{Entity, ParticleEmitterComponent, ScriptComponent, DEFAULT_SCRIPTS_DEST_DIR};

/// 3F. Add Component — Unity-style full-width pill that opens the component menu.
pub fn draw(ui: &mut egui::Ui, entity: &mut Entity) {
    ui.add_space(crate::editor::theme::from_ui(ui).space_sm);
    // A justified layout stretches the menu button to the full panel width.
    ui.with_layout(
        egui::Layout::top_down_justified(egui::Align::Center),
        |ui| {
            ui.menu_button(format!("{}  Add Component", icon::PLUS), |ui| {
                add_menu(ui, entity);
            });
        },
    );
}

fn add_menu(ui: &mut egui::Ui, entity: &mut Entity) {
    // Every project `.lua` that exposes a lifecycle table is a MonoBehaviour and
    // is offered here with the moon glyph (the per-type icons for the other
    // built-ins are the burn-down in #82); helper modules without a lifecycle
    // table are not. An entity can hold many scripts (#83), so each pick appends.
    add_script_menu(ui, entity);
    add_lighting_combat(ui, entity);
    add_physics_components(ui, entity);
    add_render_components(ui, entity);
}

/// Add-menu entries for light, health, animator and collider. Each entry is
/// offered only when absent. Animator now has its own entry (#82) — it used to be
/// created only as a side-effect of Health, which left it without an Add Component
/// axis of its own.
fn add_lighting_combat(ui: &mut egui::Ui, entity: &mut Entity) {
    if entity.light.is_none() && ui.button("Light Component").clicked() {
        entity.light = Some(authoring::default_light());
        ui.close_menu();
    }
    if entity.health.is_none() && ui.button("Health Component (Enemies)").clicked() {
        entity.health = Some(authoring::default_health());
        ui.close_menu();
    }
    if entity.animator.is_none() && ui.button("Animator Component").clicked() {
        entity.animator = Some(authoring::default_animator());
        ui.close_menu();
    }
    if entity.collider.is_none() && ui.button("Collider Component").clicked() {
        entity.collider = Some(authoring::default_collider());
        ui.close_menu();
    }
}

/// Add-menu entries for rigidbody, material/texture and nav-agent. Each entry is
/// offered only when absent.
fn add_physics_components(ui: &mut egui::Ui, entity: &mut Entity) {
    if entity.rigidbody.is_none() && ui.button("RigidBody Component").clicked() {
        entity.rigidbody = Some(authoring::default_rigidbody());
        ui.close_menu();
    }
    if entity.texture.is_none() && ui.button("Material / Texture Component").clicked() {
        entity.texture = Some(authoring::default_texture());
        ui.close_menu();
    }
    if entity.nav_agent.is_none() && ui.button("NavMesh Agent Component").clicked() {
        entity.nav_agent = Some(authoring::default_nav_agent());
        ui.close_menu();
    }
}

/// The rendering half of the Add Component menu: camera, particles and the
/// camera-only visual correction stack. Each entry is offered only when absent.
fn add_render_components(ui: &mut egui::Ui, entity: &mut Entity) {
    if entity.camera.is_none() && ui.button("Camera Component").clicked() {
        entity.camera = Some(authoring::default_camera());
        ui.close_menu();
    }
    if entity.particles.is_none()
        && ui
            .button(format!("{}  Particle System", icon::SPARKLE))
            .clicked()
    {
        entity.particles = Some(ParticleEmitterComponent::default());
        ui.close_menu();
    }
    if entity.camera.is_some()
        && entity.visual_correction.is_none()
        && ui.button("Visual Correction Component").clicked()
    {
        entity.visual_correction = Some(authoring::default_visual_correction());
        ui.close_menu();
    }
}

/// List the project's MonoBehaviour scripts (any `.lua` exposing a lifecycle
/// table) under the moon glyph; picking one appends a `ScriptComponent` to the
/// entity's `scripts` (#83). Duplicates are allowed, mirroring Unity. When the
/// project has no such scripts, a disabled hint is shown instead.
fn add_script_menu(ui: &mut egui::Ui, entity: &mut Entity) {
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
            entity.scripts.push(ScriptComponent {
                path,
                is_loaded: false,
                ..Default::default()
            });
            ui.close_menu();
        }
    }
}
