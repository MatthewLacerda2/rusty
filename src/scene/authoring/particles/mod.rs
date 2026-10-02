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

use crate::components::{
    CollisionResponse, EmitFrom, EmitMode, EmitShape, ParticleBlend, ParticleEmitterComponent,
    SubEmitTrigger,
};
use crate::core::curve::{ColorRange, Curve, Gradient, Range};

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
    p.rate = rate.max(0.0);
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
    p.texture = (!texture.is_empty()).then_some(texture);
}

/// Set the per-particle lifetime range (seconds), each bound clamped to `≥ 0`.
pub fn set_lifetime(p: &mut ParticleEmitterComponent, lifetime: Range) {
    p.lifetime = Range::new(lifetime.min.max(0.0), lifetime.max.max(0.0));
}

/// Set the per-particle launch speed range.
pub fn set_speed(p: &mut ParticleEmitterComponent, speed: Range) {
    p.speed = speed;
}

/// Set the emission direction (the shape's axis).
pub fn set_direction(p: &mut ParticleEmitterComponent, direction: Vec3) {
    p.direction = direction;
}

/// Set the emission spread.
pub fn set_spread(p: &mut ParticleEmitterComponent, spread: f32) {
    p.spread = spread;
}

/// Set the per-particle gravity.
pub fn set_gravity(p: &mut ParticleEmitterComponent, gravity: Vec3) {
    p.gravity = gravity;
}

/// Set the emission shape.
pub fn set_shape(p: &mut ParticleEmitterComponent, shape: EmitShape) {
    p.shape = shape;
}

/// Set whether the shape emits from its volume or its surface / edge.
pub fn set_emit_from(p: &mut ParticleEmitterComponent, from: EmitFrom) {
    p.emit_from = from;
}

/// Set the start size range, each bound clamped to `≥ 0`.
pub fn set_size(p: &mut ParticleEmitterComponent, size: Range) {
    p.size = Range::new(size.min.max(0.0), size.max.max(0.0));
}

/// Set the start rotation range (degrees).
pub fn set_rotation(p: &mut ParticleEmitterComponent, rotation: Range) {
    p.rotation = rotation;
}

/// Set the start colour range (RGBA, unmultiplied).
pub fn set_color(p: &mut ParticleEmitterComponent, color: ColorRange) {
    p.color = color;
}

/// Set the size-over-life multiplier curve (keys re-sorted by `t`).
pub fn set_size_over_life(p: &mut ParticleEmitterComponent, mut curve: Curve) {
    curve.sort();
    p.size_over_life = curve;
}

/// Set the colour-over-life gradient (keys re-sorted by `t`).
pub fn set_color_over_life(p: &mut ParticleEmitterComponent, mut gradient: Gradient) {
    gradient.sort();
    p.color_over_life = gradient;
}

/// Set the drag-over-life curve (fraction of velocity lost per second).
pub fn set_drag(p: &mut ParticleEmitterComponent, mut curve: Curve) {
    curve.sort();
    p.drag = curve;
}

/// Set the rotation-speed-over-life curve (degrees/second).
pub fn set_rotation_speed(p: &mut ParticleEmitterComponent, mut curve: Curve) {
    curve.sort();
    p.rotation_speed = curve;
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
    p.sub_emitters.inherit_velocity = fraction;
}

/// Set the collision response (none / die / bounce).
pub fn set_collision(p: &mut ParticleEmitterComponent, collision: CollisionResponse) {
    p.collision = collision;
}

/// Set the collision bounciness (restitution).
pub fn set_bounciness(p: &mut ParticleEmitterComponent, bounciness: f32) {
    p.bounciness = bounciness;
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
            assert_eq!(p.texture.as_deref(), Some("smoke.png"));
            assert_eq!(p.color.min, [0.1, 0.2, 0.3, 0.4]);
            assert_eq!(p.size, Range::new(0.0, 2.0), "size clamps to 0");
            assert_eq!(p.seed, 42);
        }
        set_texture(&mut e, String::new()); // empty clears
        assert_eq!(e.texture, None);
    }
}
