//! The Volumetric Fog section (#1703): the switch and, while on, the medium (mean
//! free path, albedo, anisotropy, heights, distance, the ambient's share), its
//! quality level and, at `Custom`, its grid. It sits after SSIL, with the lighting
//! sections.

use super::{count_row, knob, knob_row, quality_row, toggle, Section};
use crate::components::VolumetricFog;
use crate::core::settings::quality::EffectQuality;
use crate::scene::authoring::rendering::post_processing::volumetric_fog as fog;
use crate::scene::PostProcessingComponent;

pub(super) const SECTION: Section = Section {
    effect: "volumetric_fog",
    group: None,
    order: 53,
    draw,
};

type Fog = VolumetricFog;

fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    ui.add_space(3.0);
    ui.label("Volumetric Fog");
    let on = vc.volumetric_fog.active;
    let cx = (&mut *world, id);
    toggle(
        ui,
        cx,
        "  Volumetric Fog Active",
        on,
        fog::set_active,
        is_dirty,
    );
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
        *is_dirty |= knob_row(ui, (&mut *world, id), row, vc, range);
    }
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
        *is_dirty = true;
    }
    draw_quality(ui, world, id, vc, is_dirty);
}

/// The quality level, and at `Custom` the grid.
fn draw_quality(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let s = &vc.volumetric_fog;
    if let Some(quality) = quality_row(ui, "Volumetric Fog Quality", s.quality) {
        if let Some(mut c) = world.post_processing_mut(id) {
            fog::set_quality(&mut c, quality);
        }
        *is_dirty = true;
    }
    if s.quality != EffectQuality::Custom {
        return;
    }
    let range = Fog::MIN_RESOLUTION..=Fog::MAX_RESOLUTION;
    let (label, set) = ("    Resolution:", fog::set_resolution);
    *is_dirty |= count_row(ui, (&mut *world, id), label, s.resolution, range, set);
    let range = Fog::MIN_SLICES..=Fog::MAX_SLICES;
    let (label, set) = ("    Slices:", fog::set_slices);
    *is_dirty |= count_row(ui, (world, id), label, s.slices, range, set);
}
