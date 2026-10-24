//! src/scene/authoring/particles.rs — Shared particle-emitter-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `ParticleEmitterComponent` field by field: emission (`active` / `emit_mode` /
//! `looping` / `rate` / `burst_count` / `max_particles`), rendering (`blend` /
//! `texture`), shape + per-particle motion (`shape` / `emit_from` / `lifetime` /
//! `speed` / `direction` / `spread` / `gravity`), start size / rotation / colour
//! ranges, the over-life curves and gradient, collision (`collision` /
//! `bounciness`), sub-emitters, and the deterministic `seed`. The #440 render
//! settings (`render.*`: mode, stretch, mesh, flipbook, soft, lit) live in `render`.
//!
//! BOTH the editor's Particle System card and the Lua `Particles.*` field-setters
//! route through these, so the egui panel and the binding share one write (#287). The
//! `Particles` surface's `Emit`/`Burst`/`Clear` are emission *operations* (they call
//! the component's seeded spawn path), not field writes, so they stay in the API; its
//! setters (`SetActive`, `SetRate`, `SetShape`, `SetLifetime`, …) are field writes
//! routed here.
//!
//! Pure.

mod render;

pub use render::*;

use glam::Vec3;

use crate::asset::MapRef;
use crate::components::{
    CollisionResponse, EmitFrom, EmitMode, EmitShape, ParticleBlend, ParticleEmitterComponent,
    SubEmitTrigger,
};
use crate::core::curve::{ColorRange, Curve, Gradient, Range};
use crate::scene::authoring::sanitize::{
    clamped, color_range_or, finite, finite3, finite3_or, finite_curve, finite_gradient,
    non_negative, range_or,
};

/// Set the emitter's `active` gate.
pub fn set_active(p: &mut ParticleEmitterComponent, active: bool) {
    p.active = active;
}

/// Set the emission mode (continuous / burst).
pub fn set_emit_mode(p: &mut ParticleEmitterComponent, mode: EmitMode) {
    p.emit_mode = mode;
}

/// Set whether the emitter loops.
pub fn set_looping(p: &mut ParticleEmitterComponent, looping: bool) {
    p.looping = looping;
}

/// Set the continuous emission rate, clamped to `≥ 0` (matches `Particles.SetRate`).
pub fn set_rate(p: &mut ParticleEmitterComponent, rate: f32) {
    p.rate = non_negative(rate);
}

/// Set the per-burst particle count.
pub fn set_burst_count(p: &mut ParticleEmitterComponent, burst_count: u32) {
    p.burst_count = burst_count;
}

/// Set the live-particle cap.
pub fn set_max_particles(p: &mut ParticleEmitterComponent, max_particles: u32) {
    p.max_particles = max_particles;
}

/// Set the blend mode (alpha / additive).
pub fn set_blend(p: &mut ParticleEmitterComponent, blend: ParticleBlend) {
    p.blend = blend;
}

/// Set (or, with an empty string, clear) the particle texture path.
pub fn set_texture(p: &mut ParticleEmitterComponent, texture: String) {
    p.texture = (!texture.is_empty()).then(|| MapRef::from(texture));
}

/// Set the per-particle lifetime range (seconds), each bound clamped to `≥ 0`.
pub fn set_lifetime(p: &mut ParticleEmitterComponent, lifetime: Range) {
    let r = range_or(lifetime, p.lifetime);
    p.lifetime = Range::new(non_negative(r.min), non_negative(r.max));
}

/// Set the per-particle launch speed range.
pub fn set_speed(p: &mut ParticleEmitterComponent, speed: Range) {
    p.speed = range_or(speed, p.speed);
}

/// Set the emission direction (the shape's axis).
pub fn set_direction(p: &mut ParticleEmitterComponent, direction: Vec3) {
    p.direction = finite3_or(direction, p.direction);
}

/// Set the emission spread.
pub fn set_spread(p: &mut ParticleEmitterComponent, spread: f32) {
    p.spread = non_negative(spread);
}

/// Set the per-particle gravity.
pub fn set_gravity(p: &mut ParticleEmitterComponent, gravity: Vec3) {
    p.gravity = finite3(gravity);
}

/// Set the emission shape: radii `≥ 0`, a box's size `≥ 0` (a non-finite size is
/// zero), a cone's angle in `[0, 90]` degrees (Unity's shape module range).
// lint: allow(sanitize) each variant's floats go through a helper in the match
pub fn set_shape(p: &mut ParticleEmitterComponent, shape: EmitShape) {
    p.shape = match shape {
        EmitShape::Point => EmitShape::Point,
        EmitShape::Sphere { radius } => EmitShape::Sphere {
            radius: non_negative(radius),
        },
        EmitShape::Hemisphere { radius } => EmitShape::Hemisphere {
            radius: non_negative(radius),
        },
        EmitShape::Box { size } => EmitShape::Box {
            size: finite3(size).max(Vec3::ZERO),
        },
        EmitShape::Cone { angle, radius } => EmitShape::Cone {
            angle: clamped(angle, 0.0, 90.0),
            radius: non_negative(radius),
        },
        EmitShape::Circle { radius } => EmitShape::Circle {
            radius: non_negative(radius),
        },
    };
}

/// Set whether the shape emits from its volume or its surface / edge.
pub fn set_emit_from(p: &mut ParticleEmitterComponent, from: EmitFrom) {
    p.emit_from = from;
}

/// Set the start size range, each bound clamped to `≥ 0`.
pub fn set_size(p: &mut ParticleEmitterComponent, size: Range) {
    let r = range_or(size, p.size);
    p.size = Range::new(non_negative(r.min), non_negative(r.max));
}

/// Set the start rotation range (degrees).
pub fn set_rotation(p: &mut ParticleEmitterComponent, rotation: Range) {
    p.rotation = range_or(rotation, p.rotation);
}

/// Set the start colour range (RGBA, unmultiplied).
pub fn set_color(p: &mut ParticleEmitterComponent, color: ColorRange) {
    p.color = color_range_or(color, p.color);
}

/// Set the size-over-life multiplier curve (keys re-sorted by `t`, non-finite
/// keys dropped).
pub fn set_size_over_life(p: &mut ParticleEmitterComponent, curve: Curve) {
    p.size_over_life = finite_curve(curve);
}

/// Set the colour-over-life gradient (keys re-sorted by `t`).
pub fn set_color_over_life(p: &mut ParticleEmitterComponent, gradient: Gradient) {
    p.color_over_life = finite_gradient(gradient);
}

/// Set the drag-over-life curve (fraction of velocity lost per second).
pub fn set_drag(p: &mut ParticleEmitterComponent, curve: Curve) {
    p.drag = finite_curve(curve);
}

/// Set the rotation-speed-over-life curve (degrees/second).
pub fn set_rotation_speed(p: &mut ParticleEmitterComponent, curve: Curve) {
    p.rotation_speed = finite_curve(curve);
}

/// Set (or, with `None`, clear) the sub-emitter fired at `trigger`.
pub fn set_sub_emitter(
    p: &mut ParticleEmitterComponent,
    trigger: SubEmitTrigger,
    target: Option<u32>,
) {
    p.sub_emitters.set(trigger, target);
}

/// Set the fraction of a parent particle's velocity sub-particles inherit.
pub fn set_inherit_velocity(p: &mut ParticleEmitterComponent, fraction: f32) {
    p.sub_emitters.inherit_velocity = finite(fraction);
}

/// Set the collision response (none / die / bounce).
pub fn set_collision(p: &mut ParticleEmitterComponent, collision: CollisionResponse) {
    p.collision = collision;
}

/// Set the collision bounciness (restitution).
pub fn set_bounciness(p: &mut ParticleEmitterComponent, bounciness: f32) {
    p.bounciness = non_negative(bounciness);
}

/// Set the deterministic spawn seed.
pub fn set_seed(p: &mut ParticleEmitterComponent, seed: u64) {
    p.seed = seed;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ParticleEmitterComponent;
    use crate::scene::Scene;

    fn scene_with_particles() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Emitter".to_string());
        scene
            .world
            .set_particles(id, Some(ParticleEmitterComponent::default()));
        (scene, id)
    }

    #[test]
    fn ops_write_through_with_rate_clamp_and_texture_clear() {
        let (mut scene, id) = scene_with_particles();
        let mut e = scene.world.particles_mut(id).unwrap();
        set_active(&mut e, true);
        set_rate(&mut e, -5.0);
        set_emit_mode(&mut e, EmitMode::Burst);
        set_texture(&mut e, "smoke.png".to_string());
        set_color(&mut e, ColorRange::constant([0.1, 0.2, 0.3, 0.4]));
        set_size(&mut e, Range::new(-1.0, 2.0));
        set_seed(&mut e, 42);
        {
            let p = &*e;
            assert!(p.active);
            assert_eq!(p.rate, 0.0, "rate clamps to 0");
            assert_eq!(p.emit_mode, EmitMode::Burst);
            assert_eq!(p.texture, Some("smoke.png".into()));
            assert_eq!(p.color.min, [0.1, 0.2, 0.3, 0.4]);
            assert_eq!(p.size, Range::new(0.0, 2.0), "size clamps to 0");
            assert_eq!(p.seed, 42);
        }
        set_texture(&mut e, String::new()); // empty clears
        assert_eq!(e.texture, None);
    }
}
