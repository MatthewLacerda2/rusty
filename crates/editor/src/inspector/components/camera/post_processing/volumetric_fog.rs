//! The Volumetric Fog section: the switch and, while on, the medium (mean
//! free path, albedo, anisotropy, heights, distance, the ambient's share), its
//! quality level and, at `Custom`, its grid. It sits after SSIL, with the lighting
//! sections.

use super::{count_row, knob, knob_row, quality_row, toggle, Section};
use rusty_components::VolumetricFog;
use rusty_core::settings::quality::EffectQuality;
use rusty_edit::authoring::rendering::post_processing::volumetric_fog as fog;
use rusty_scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "volumetric_fog",
    group: None,
    order: 53,
    draw,
};

type Fog = VolumetricFog;

fn draw(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32, vc: &PostProcessingComponent) {
    ui.add_space(3.0);
    ui.label("Volumetric Fog");
    let on = vc.volumetric_fog.active;
    let cx = (&mut *world, id);
    toggle(ui, cx, "  Volumetric Fog Active", on, fog::set_active);
    if !on {
        return;
    }
    let knobs = fog::EFFECT.knobs;
    let h = Fog::MAX_HEIGHT;
    let rows = [
        (
            "    Fog Attenuation Distance:",
            "mean_free_path",
            Fog::MIN_MEAN_FREE_PATH..=1000.0,
        ),
        (
            "    Anisotropy:",
            "anisotropy",
            -Fog::MAX_ANISOTROPY..=Fog::MAX_ANISOTROPY,
        ),
        ("    Base Height:", "base_height", -h..=h),
        ("    Maximum Height:", "maximum_height", -h..=h),
        (
            "    Volumetric Fog Distance:",
            "depth_extent",
            Fog::MIN_DEPTH_EXTENT..=Fog::MAX_DEPTH_EXTENT,
        ),
        ("    GI Dimmer:", "global_light_probe_dimmer", 0.0..=1.0),
    ];
    for (label, field, range) in rows {
        let row = (label, knob(knobs, field));
        knob_row(ui, (&mut *world, id), row, vc, range);
    }
    draw_albedo(ui, world, id, vc);
    draw_quality(ui, world, id, vc);
}

/// The albedo's colour picker.
fn draw_albedo(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
) {
    let mut albedo = vc.volumetric_fog.albedo;
    let changed = ui
        .horizontal(|ui| {
            ui.label("    Albedo:");
            ui.color_edit_button_rgb(&mut albedo).changed()
        })
        .inner;
    if changed {
        if let Some(mut c) = world.post_processing_mut(id) {
            fog::set_albedo(&mut c, albedo);
        }
    }
}

/// The quality level, and at `Custom` the grid.
fn draw_quality(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
) {
    let s = &vc.volumetric_fog;
    if let Some(quality) = quality_row(ui, "Volumetric Fog Quality", s.quality) {
        if let Some(mut c) = world.post_processing_mut(id) {
            fog::set_quality(&mut c, quality);
        }
    }
    if s.quality != EffectQuality::Custom {
        return;
    }
    let range = Fog::MIN_RESOLUTION..=Fog::MAX_RESOLUTION;
    let (label, set) = ("    Resolution:", fog::set_resolution);
    count_row(ui, (&mut *world, id), label, s.resolution, range, set);
    let range = Fog::MIN_SLICES..=Fog::MAX_SLICES;
    let (label, set) = ("    Slices:", fog::set_slices);
    count_row(ui, (world, id), label, s.slices, range, set);
}
