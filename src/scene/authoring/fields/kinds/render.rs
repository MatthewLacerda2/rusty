//! src/scene/authoring/fields/kinds/render.rs — the rendering components' field tables:
//! `Transform`, `Mesh`, `Material`, `Light`, `Camera`, `PostProcessing` and
//! `LODGroup`.

use super::super::table::{group, inspect, op, part, ro, unheld, with, Group};
use super::super::value::Mask;
use super::NO_OP;
use crate::components::rendering::camera::RenderTarget;
use crate::components::{
    Bloom, CameraComponent, ChromaticAberration, ColorAdjustments, ColorLookup, CustomEffects,
    DepthOfField, Exposure, FilmGrain, LiftGammaGain, LightComponent, LodGroupComponent, LodLevel,
    MaterialComponent, MeshComponent, PostProcessingComponent, ShadowSettings, SsaoSettings, Ssr,
    Tonemapping, TransformComponent, Vignette, Wheel, WhiteBalance,
};
use crate::scene::authoring::{
    rendering::camera, rendering::light, rendering::lod_group as lod,
    rendering::post_processing as vc,
};
use crate::shadergen::post_params::PostChain;

const CONES: Group<LightComponent> =
    group!(|l, inner_cone: f32, outer_cone: f32| light::set_cones(l, inner_cone, outer_cone));

const MESH: &str = "set by the model or primitive the entity was made from";

inspect! {
    TransformComponent => transform, transform_mut, &[
        ro!("position", "Transform.SetLocalPosition / SetPosition write it"),
        ro!("rotation", "Transform.SetLocalRotation / SetRotation write it"),
        ro!("scale", "Transform.SetLocalScale writes it"),
    ];
    MeshComponent => mesh, mesh_mut, &[
        ro!("primitive_type", MESH),
        ro!("asset_ref", MESH),
        ro!("skeleton", MESH),
    ];
    MaterialComponent => material, material_mut, &[
        ro!("materials", "Material.Set points a slot at a library material"),
    ];
    LightComponent => light, light_mut, &[
        op!("light_type", light::set_type),
        op!("color", light::set_color),
        op!("intensity", light::set_intensity),
        op!("range", light::set_range),
        part!("inner_cone", CONES),
        part!("outer_cone", CONES),
        op!("cast_shadows", light::set_cast_shadows),
        op!("mode", light::set_mode),
    ];
    CameraComponent => camera, camera_mut, &[
        ro!("active", NO_OP),
        op!("fov", camera::set_fov),
        op!("near", camera::set_near),
        op!("far", camera::set_far),
        with!("culling_mask", |c, v: Mask| camera::set_culling_mask(c, v.0)),
        op!("render_order", camera::set_render_order),
        op!("clear_flags", camera::set_clear_flags),
        op!("motion_blur_active", camera::set_motion_blur_active),
        op!("motion_blur_samples", camera::set_motion_blur_samples),
        op!("motion_blur_intensity", camera::set_motion_blur_intensity),
        op!("motion_blur_max_length", camera::set_motion_blur_max_length),
        op!("fxaa_active", camera::set_fxaa_active),
        op!("projection", camera::set_projection),
        with!("target_texture", |c, v: Option<RenderTarget>| set_target(c, v)),
    ];
    PostProcessingComponent => post_processing, post_processing_mut, &[
        op!("active", vc::set_active),
        with!("bloom", |c, v: Bloom| {
            vc::set_bloom_active(c, v.active);
            vc::set_bloom_intensity(c, v.intensity);
            vc::set_bloom_threshold(c, v.threshold);
        }),
        with!("chromatic_aberration", |c, v: ChromaticAberration| {
            vc::set_chromatic_aberration_active(c, v.active);
            vc::set_chromatic_aberration_intensity(c, v.intensity);
        }),
        with!("color_adjustments", |c, v: ColorAdjustments| {
            vc::set_contrast(c, v.contrast);
            vc::set_saturation(c, v.saturation);
            vc::set_hue_shift(c, v.hue_shift);
        }),
        with!("color_lookup", |c, v: ColorLookup| {
            vc::set_lut(c, v.lut);
            vc::set_lut_contribution(c, v.contribution);
        }),
        unheld!(
            "its op reads every effect's baked sidecar and warns on an unbaked one",
            with!("custom_effects", |c, v: CustomEffects| {
                vc::set_custom_effects(c, v.names)?;
                let chain = PostChain::load(&c.custom_effects.names);
                for (name, value) in v.params {
                    vc::set_post_param_with(c, &chain, &name, value)?;
                }
            })
        ),
        with!("depth_of_field", |c, v: DepthOfField| {
            vc::set_dof_mode(c, v.mode);
            for knob in vc::DOF_KNOBS {
                (knob.set)(c, (knob.get)(&v));
            }
        }),
        with!("exposure", |c, v: Exposure| {
            vc::set_exposure(c, v.fixed_ev);
            vc::set_exposure_mode(c, v.mode);
            let a = v.automatic;
            vc::set_exposure_limits(c, a.min_ev, a.max_ev);
            vc::set_exposure_compensation(c, a.compensation);
            vc::set_adaptation_speed(c, a.speed_up, a.speed_down);
            vc::set_histogram_percent(c, a.low_percent, a.high_percent);
        }),
        with!("film_grain", |c, v: FilmGrain| {
            vc::set_film_grain_active(c, v.active);
            vc::set_film_grain_intensity(c, v.intensity);
            vc::set_film_grain_response(c, v.response);
        }),
        with!("lift_gamma_gain", |c, v: LiftGammaGain| {
            for w in Wheel::ALL {
                vc::set_wheel(c, w, v.wheel(w).color, v.wheel(w).offset);
            }
        }),
        with!("ssr", |c, v: Ssr| {
            vc::set_ssr_active(c, v.active);
            vc::set_ssr_quality(c, v.quality);
            vc::set_ssr_temporal_upsampling(c, v.temporal_upsampling);
        }),
        with!("tonemapping", |c, v: Tonemapping| {
            vc::set_tonemap(c, v.mode);
            vc::set_gamma(c, v.display_gamma);
        }),
        with!("vignette", |c, v: Vignette| {
            vc::set_vignette_active(c, v.active);
            vc::set_vignette_intensity(c, v.intensity);
            vc::set_vignette_smoothness(c, v.smoothness);
            vc::set_vignette_color(c, v.color);
            vc::set_vignette_center(c, v.center);
        }),
        with!("white_balance", |c, v: WhiteBalance| {
            vc::set_white_balance(c, v.temperature, v.tint);
        }),
        with!("shadows", |c, v: ShadowSettings| {
            vc::set_shadow_cascades(c, v.cascades);
            vc::set_shadow_distance(c, v.distance);
        }),
        with!("ssao", |c, v: SsaoSettings| {
            vc::set_ssao_active(c, v.active);
            vc::set_ssao_radius(c, v.radius);
            vc::set_ssao_intensity(c, v.intensity);
        }),
    ];
    LodGroupComponent => lod_group, lod_group_mut, &[
        with!("levels", |g, v: Vec<LodLevel>| set_levels(g, v)),
        op!("size", lod::set_size),
    ];
}

/// A camera's render target, whole: `None` draws to the screen again.
fn set_target(c: &mut CameraComponent, target: Option<RenderTarget>) {
    let Some(t) = target else {
        return camera::set_target_texture(c, "", 0, 0);
    };
    camera::set_target_texture(c, &t.name, t.width, t.height);
    camera::set_target_post_fx(c, t.post_fx);
    camera::set_target_update_every(c, t.update_every);
}

/// The LOD levels, whole: the count through add/remove, then each level's
/// renderers and threshold. A threshold is clamped between its neighbours, so
/// they are first dropped to zero, coarsest first, and then set finest first: each
/// one is then clamped only by the finer threshold just set. Two passes, not one
/// per level.
fn set_levels(g: &mut LodGroupComponent, levels: Vec<LodLevel>) {
    while g.levels.len() > levels.len() {
        lod::remove_level(g, g.levels.len() - 1);
    }
    while g.levels.len() < levels.len() {
        lod::add_level(g);
    }
    for i in (0..levels.len()).rev() {
        lod::set_level_height(g, i, 0.0);
    }
    for (i, level) in levels.iter().enumerate() {
        lod::set_level_height(g, i, level.screen_height);
    }
    for (i, level) in levels.into_iter().enumerate() {
        lod::set_renderers(g, i, level.renderers);
    }
}
