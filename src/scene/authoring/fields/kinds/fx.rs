//! src/scene/authoring/fields/kinds/fx.rs — the animation, effects and audio components'
//! field tables: `Animator`, `Particles`, `Audio`, `AudioReverbZone`, `Trail` and
//! `Line`.

use glam::Vec3;

use super::super::table::{inspect, op, ro, with};
use crate::components::{
    AnimatorComponent, AudioSourceComponent, LineComponent, ParticleEmitterComponent,
    ParticleRender, ReverbZoneComponent, RibbonStyle, SubEmitTrigger, SubEmitters, TrailComponent,
};
use crate::scene::authoring::particles as fx;
use crate::scene::authoring::{
    animator, audio::reverb_zone as reverb, audio::source as audio, ribbon, ribbon::line,
    ribbon::trail,
};

const PLAYBACK: &str =
    "playback state the animation step advances (Animator.Play / PlayNode drive it)";

inspect! {
    AnimatorComponent => animator, animator_mut, &[
        op!("current_clip", animator::set_clip),
        ro!("current_tree", PLAYBACK),
        ro!("time", PLAYBACK),
        op!("loop_clip", animator::set_looping),
        ro!("ended", PLAYBACK),
        ro!("previous_clip", PLAYBACK),
        ro!("previous_tree", PLAYBACK),
        ro!("previous_time", PLAYBACK),
        ro!("previous_loop", PLAYBACK),
        ro!("crossfade_elapsed", PLAYBACK),
        ro!("crossfade_duration", PLAYBACK),
        ro!("current_node", PLAYBACK),
        ro!("node_speed", PLAYBACK),
        op!("speed", animator::set_speed),
        op!("is_playing", animator::set_playing),
        op!("freeze", animator::set_freeze),
        ro!("parameters", "Animator.SetFloat / SetInt / SetBool / SetTrigger write them"),
        // `""` is no graph, as `Animator.SetGraph` and the card read it.
        with!("graph", |a, v: Option<String>| {
            animator::set_graph(a, v.filter(|p| !p.is_empty()))
        }),
        op!("graph_enabled", animator::set_graph_enabled),
        ro!("layers", "Animator.SetLayerWeight writes a layer's weight"),
        ro!("ik", "Animator.AddTwoBoneIK / AddAimIK / SetIKWeight write them"),
    ];
    ParticleEmitterComponent => particles, particles_mut, &[
        op!("active", fx::set_active),
        op!("emit_mode", fx::set_emit_mode),
        op!("blend", fx::set_blend),
        op!("collision", fx::set_collision),
        with!("texture", |p, v: Option<String>| fx::set_texture(p, v.unwrap_or_default())),
        op!("rate", fx::set_rate),
        op!("burst_count", fx::set_burst_count),
        op!("max_particles", fx::set_max_particles),
        op!("looping", fx::set_looping),
        op!("shape", fx::set_shape),
        op!("emit_from", fx::set_emit_from),
        op!("lifetime", fx::set_lifetime),
        op!("speed", fx::set_speed),
        op!("direction", fx::set_direction),
        op!("spread", fx::set_spread),
        op!("gravity", fx::set_gravity),
        op!("size", fx::set_size),
        op!("rotation", fx::set_rotation),
        op!("color", fx::set_color),
        op!("size_over_life", fx::set_size_over_life),
        op!("color_over_life", fx::set_color_over_life),
        op!("drag", fx::set_drag),
        op!("rotation_speed", fx::set_rotation_speed),
        op!("bounciness", fx::set_bounciness),
        with!("sub_emitters", |p, v: SubEmitters| {
            fx::set_sub_emitter(p, SubEmitTrigger::Birth, v.birth);
            fx::set_sub_emitter(p, SubEmitTrigger::Death, v.death);
            fx::set_sub_emitter(p, SubEmitTrigger::Collision, v.collision);
            fx::set_inherit_velocity(p, v.inherit_velocity);
        }),
        op!("seed", fx::set_seed),
        with!("render", |p, v: ParticleRender| set_render(p, v)),
    ];
    AudioSourceComponent => audio, audio_mut, &[
        op!("clip", audio::set_clip),
        op!("volume", audio::set_volume),
        op!("loop", audio::set_looping),
        op!("play_on_start", audio::set_play_on_start),
        op!("is_time_scaled", audio::set_time_scaled),
        op!("spatial_blend", audio::set_spatial_blend),
        op!("initial_distance", audio::set_initial_distance),
        op!("final_distance", audio::set_final_distance),
        op!("output_group", audio::set_output_group),
        op!("occlusion_enabled", audio::set_occlusion_enabled),
    ];
    ReverbZoneComponent => reverb_zone, reverb_zone_mut, &[
        op!("min_distance", reverb::set_min_distance),
        op!("max_distance", reverb::set_max_distance),
        // `params` first: writing them makes the preset `Custom`, so a preset
        // named beside them is applied after (and loads its own params).
        op!("params", reverb::set_params),
        op!("preset", reverb::set_preset),
    ];
    TrailComponent => trail, trail_mut, &[
        op!("emitting", trail::set_emitting),
        op!("time", trail::set_time),
        op!("min_vertex_distance", trail::set_min_vertex_distance),
        with!("style", |t, v: RibbonStyle| set_style(&mut t.style, v)),
    ];
    LineComponent => line, line_mut, &[
        with!("positions", |l, v: Vec<Vec3>| line::set_positions(l, &v)),
        op!("use_world_space", line::set_use_world_space),
        op!("looping", line::set_looping),
        with!("style", |l, v: RibbonStyle| set_style(&mut l.style, v)),
    ];
}

/// An emitter's render settings, whole.
fn set_render(p: &mut ParticleEmitterComponent, r: ParticleRender) {
    fx::set_render_mode(p, r.mode);
    fx::set_stretch(p, r.length_scale, r.speed_scale);
    fx::set_render_mesh(p, r.mesh.unwrap_or_default());
    fx::set_render_material(p, r.material.unwrap_or_default());
    fx::set_flipbook(p, r.flipbook);
    fx::set_soft_distance(p, r.soft_distance);
    fx::set_lit(p, r.lit);
}

/// A trail's or line's ribbon look, whole.
fn set_style(s: &mut RibbonStyle, v: RibbonStyle) {
    ribbon::set_width(s, v.width);
    ribbon::set_color(s, v.color);
    ribbon::set_texture(
        s,
        v.texture.map(|t| t.as_str().to_owned()).unwrap_or_default(),
    );
    ribbon::set_texture_mode(s, v.texture_mode);
    ribbon::set_blend(s, v.blend);
}
