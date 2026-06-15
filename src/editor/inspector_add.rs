use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::scene::{
    AnimatorComponent, CameraComponent, ColliderComponent, ColliderShape, Entity, HealthComponent,
    LightComponent, LightType, NavMeshAgentComponent, RigidBodyComponent, ScriptComponent,
    TextureComponent, Tonemap, VisualCorrectionComponent,
};

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
    if entity.script.is_none() && ui.button("Script Component").clicked() {
        entity.script = Some(ScriptComponent {
            path: "project/assets/scripts/bot.lua".to_string(),
            is_loaded: false,
        });
        ui.close_menu();
    }
    if entity.light.is_none() && ui.button("Light Component").clicked() {
        entity.light = Some(LightComponent {
            light_type: LightType::Point,
            color: Vec3::ONE,
            intensity: 1.5,
            range: 10.0,
            inner_cone: 30.0,
            outer_cone: 45.0,
        });
        ui.close_menu();
    }
    if entity.health.is_none() && ui.button("Health Component (Enemies)").clicked() {
        entity.health = Some(HealthComponent {
            current_health: 100.0,
            max_health: 100.0,
            is_dead: false,
        });
        entity.animator = Some(AnimatorComponent {
            current_clip: "Idle".to_string(),
            time: 0.0,
            speed: 2.0,
            is_playing: true,
            freeze: false,
        });
        ui.close_menu();
    }
    if entity.collider.is_none() && ui.button("Collider Component").clicked() {
        entity.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box { size: Vec3::ONE },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        });
        ui.close_menu();
    }
    if entity.rigidbody.is_none() && ui.button("RigidBody Component").clicked() {
        entity.rigidbody = Some(RigidBodyComponent {
            active: true,
            is_kinematic: false,
            mass: 1.0,
            velocity: Vec3::ZERO,
            use_gravity: true,
        });
        ui.close_menu();
    }
    if entity.texture.is_none() && ui.button("Material / Texture Component").clicked() {
        entity.texture = Some(TextureComponent {
            path: "".to_string(),
            is_dirty: true,
            metallic: 0.0,
            roughness: 0.5,
            metallic_map: None,
            roughness_map: None,
            color: [1.0, 1.0, 1.0],
        });
        ui.close_menu();
    }
    if entity.nav_agent.is_none() && ui.button("NavMesh Agent Component").clicked() {
        entity.nav_agent = Some(NavMeshAgentComponent {
            active: true,
            radius: 0.5,
            target: Vec3::new(8.0, 1.0, 8.0),
            speed: 3.0,
            acceleration: 5.0,
            stopping_distance: 0.5,
            velocity: Vec3::ZERO,
        });
        ui.close_menu();
    }
    if entity.camera.is_none() && ui.button("Camera Component").clicked() {
        entity.camera = Some(CameraComponent {
            active: true,
            fov: 45.0,
            near: 0.1,
            far: 200.0,
            motion_blur_active: true,
            motion_blur_samples: 64,
        });
        ui.close_menu();
    }
    if entity.camera.is_some()
        && entity.visual_correction.is_none()
        && ui.button("Visual Correction Component").clicked()
    {
        entity.visual_correction = Some(VisualCorrectionComponent {
            active: true,
            bloom_active: true,
            bloom_intensity: 1.0,
            bloom_threshold: 0.8,
            exposure: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            ssr_active: true,
            ssr_quality: "High".to_string(),
            ssr_temporal_upsampling: true,
            tonemap: Tonemap::Aces,
            gamma: 2.2,
        });
        ui.close_menu();
    }
}
