use crate::editor::inspector::components::card::remove_component;
use crate::scene::authoring::ComponentKind;
use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::rendering::light as light_ops;
use crate::scene::{LightComponent, LightMode, LightType};

/// 3C. Light configuration. A THIN client (#287): every widget reads the field into
/// a local and, on change, routes the write through the shared `authoring::rendering::light::*`
/// op — never mutating `LightComponent` fields directly. Remove detaches the
/// reference (allowed: not a field mutation).
pub fn draw_light(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(light) = world.light(id).map(|l| l.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::LIGHTBULB, "Light", Some(&mut remove), |ui| {
        draw_light_type(ui, world, id, &light, is_dirty);
        draw_light_mode(ui, world, id, &light, is_dirty);

        let mut color_arr = [light.color.x, light.color.y, light.color.z];
        ui.horizontal(|ui| {
            ui.label("Color:");
            if ui.color_edit_button_rgb(&mut color_arr).changed() {
                if let Some(mut l) = world.light_mut(id) {
                    light_ops::set_color(&mut l, Vec3::from_array(color_arr));
                }
                *is_dirty = true;
            }
        });

        let mut intensity = light.intensity;
        ui.horizontal(|ui| {
            ui.label("Intensity:");
            if ui
                .add(crate::editor::widgets::slider(&mut intensity, 0.0..=20.0))
                .changed()
            {
                if let Some(mut l) = world.light_mut(id) {
                    light_ops::set_intensity(&mut l, intensity);
                }
                *is_dirty = true;
            }
        });

        if light.light_type == LightType::Point || light.light_type == LightType::Spotlight {
            draw_local_light(ui, world, id, &light, is_dirty);
        }

        if light.light_type == LightType::Area {
            draw_area(ui, world, id, &light, is_dirty);
        }

        if light.light_type == LightType::Spotlight {
            draw_spot_cones(ui, world, id, &light, is_dirty);
        }
    });
    if remove {
        remove_component(world, id, ComponentKind::Light, is_dirty);
    }
}

/// A point or spot light's reach and whether it casts shadows (#468).
fn draw_local_light(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    light: &LightComponent,
    is_dirty: &mut bool,
) {
    let mut range = light.range;
    ui.horizontal(|ui| {
        ui.label("Distance:");
        if ui
            .add(crate::editor::widgets::slider(&mut range, 0.1..=100.0))
            .changed()
        {
            if let Some(mut l) = world.light_mut(id) {
                light_ops::set_range(&mut l, range);
            }
            *is_dirty = true;
        }
    });
    let mut shadows = light.cast_shadows;
    if ui.checkbox(&mut shadows, "Cast Shadows").changed() {
        if let Some(mut l) = world.light_mut(id) {
            light_ops::set_cast_shadows(&mut l, shadows);
        }
        *is_dirty = true;
    }
}

/// An Area light's width, height and range (#1674). It casts no realtime shadow:
/// its bake shadows it.
fn draw_area(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    light: &LightComponent,
    is_dirty: &mut bool,
) {
    let (mut w, mut h, mut range) = (light.area_size.x, light.area_size.y, light.range);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Width:");
        changed |= ui
            .add(crate::editor::widgets::slider(&mut w, 0.01..=10.0))
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("Height:");
        changed |= ui
            .add(crate::editor::widgets::slider(&mut h, 0.01..=10.0))
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("Distance:");
        changed |= ui
            .add(crate::editor::widgets::slider(&mut range, 0.1..=100.0))
            .changed();
    });
    if changed {
        if let Some(mut l) = world.light_mut(id) {
            light_ops::set_area_size(&mut l, glam::Vec2::new(w, h));
            light_ops::set_range(&mut l, range);
        }
        *is_dirty = true;
    }
}

/// The light-type selector combo box, offering every type under its one name. The
/// shared `set_type` seeds a spot's cones when they are still zero. `light` is a
/// snapshot of the current values; writes route through the shared ops.
fn draw_light_type(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    light: &LightComponent,
    is_dirty: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label("Type:");
        egui::ComboBox::from_id_salt("LightTypeSelector")
            .selected_text(light.light_type.name())
            .show_ui(ui, |ui| {
                for light_type in LightType::ALL {
                    if ui
                        .selectable_label(light.light_type == light_type, light_type.name())
                        .clicked()
                    {
                        if let Some(mut l) = world.light_mut(id) {
                            light_ops::set_type(&mut l, light_type);
                        }
                        *is_dirty = true;
                    }
                }
            });
    });
}

/// The Realtime / Mixed / Baked selector (#438), Unity's Light "Mode" field.
fn draw_light_mode(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    light: &LightComponent,
    is_dirty: &mut bool,
) {
    if light.light_type == LightType::Area {
        // Unity's built-in pipeline bakes area lights only (#1674).
        ui.label("Mode: Baked (Area lights bake only)");
        return;
    }
    ui.horizontal(|ui| {
        ui.label("Mode:");
        egui::ComboBox::from_id_salt("LightModeSelector")
            .selected_text(light.mode.name())
            .show_ui(ui, |ui| {
                for mode in LightMode::ALL {
                    if ui
                        .selectable_label(light.mode == mode, mode.name())
                        .clicked()
                    {
                        if let Some(mut l) = world.light_mut(id) {
                            light_ops::set_mode(&mut l, mode);
                        }
                        *is_dirty = true;
                    }
                }
            });
    });
}

/// The spotlight cone sliders: outer (FOV) and inner cone. The shared op keeps the
/// inner cone from exceeding the outer one; here we read both from the snapshot,
/// edit a local, and route the pair through `set_cones`.
fn draw_spot_cones(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    light: &LightComponent,
    is_dirty: &mut bool,
) {
    let mut outer = light.outer_cone;
    ui.horizontal(|ui| {
        ui.label("FOV (Outer Cone):");
        if ui
            .add(crate::editor::widgets::slider(&mut outer, 0.1..=90.0).suffix("°"))
            .changed()
        {
            // Outer shrank below inner ⇒ pull inner down with it (op enforces it).
            if let Some(mut l) = world.light_mut(id) {
                light_ops::set_cones(&mut l, light.inner_cone, outer);
            }
            *is_dirty = true;
        }
    });
    let mut inner = light.inner_cone;
    ui.horizontal(|ui| {
        ui.label("Inner Cone:");
        if ui
            .add(crate::editor::widgets::slider(&mut inner, 0.0..=90.0).suffix("°"))
            .changed()
        {
            // Inner grew past outer ⇒ raise outer to match (mirrors the old card).
            let outer = light.outer_cone.max(inner);
            if let Some(mut l) = world.light_mut(id) {
                light_ops::set_cones(&mut l, inner, outer);
            }
            *is_dirty = true;
        }
    });
}
