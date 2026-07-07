//! src/scene/authoring/particles.rs — Shared particle-emitter-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `ParticleEmitterComponent` field by field: emission (`active` / `emit_mode` /
//! `looping` / `rate` / `burst_count` / `max_particles`), rendering (`blend` /
//! `texture`), per-particle motion (`lifetime` / `speed` / `direction` / `spread` /
//! `gravity`), size + colour over life, collision (`collision` / `bounciness`), and
//! the deterministic `seed`.
//!
//! BOTH the editor's Particle System card and the Lua `Particles.*` field-setters
//! route through these, so the egui panel and the binding share one write (#287). The
//! `Particles` surface's `Emit`/`Burst`/`Clear` are emission *operations* (they call
//! the component's seeded spawn path), not field writes, so they stay in the API; only
//! `SetActive` ([`set_active`]) and `SetRate` ([`set_rate`], with its `≥ 0` clamp) are
//! field writes routed here.
//!
//! Allowed deps: components (the emitter data + its enums). Pure.

use glam::Vec3;

use crate::components::{CollisionResponse, EmitMode, ParticleBlend, ParticleEmitterComponent};

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

/// Set the per-particle lifetime (seconds).
pub fn set_lifetime(p: &mut ParticleEmitterComponent, lifetime: f32) {
    p.lifetime = lifetime;
}

/// Set the per-particle launch speed.
pub fn set_speed(p: &mut ParticleEmitterComponent, speed: f32) {
    p.speed = speed;
}

/// Set the emission direction.
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

/// Set the start size.
pub fn set_size_start(p: &mut ParticleEmitterComponent, size_start: f32) {
    p.size_start = size_start;
}

/// Set the end size.
pub fn set_size_end(p: &mut ParticleEmitterComponent, size_end: f32) {
    p.size_end = size_end;
}

/// Set the particle colour (RGBA, unmultiplied).
pub fn set_color(p: &mut ParticleEmitterComponent, color: [f32; 4]) {
    p.color = color;
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
        scene.world.set_particles(id, Some(ParticleEmitterComponent::default()));
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
        set_color(&mut e, [0.1, 0.2, 0.3, 0.4]);
        set_seed(&mut e, 42);
        {
            let p = &*e;
            assert!(p.active);
            assert_eq!(p.rate, 0.0, "rate clamps to 0");
            assert_eq!(p.emit_mode, EmitMode::Burst);
            assert_eq!(p.texture.as_deref(), Some("smoke.png"));
            assert_eq!(p.color, [0.1, 0.2, 0.3, 0.4]);
            assert_eq!(p.seed, 42);
        }
        set_texture(&mut e, String::new()); // empty clears
        assert_eq!((&*e).texture, None);
    }
}
