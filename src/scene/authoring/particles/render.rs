//! src/scene/authoring/particles/render.rs — the emitter's render-settings ops
//! (#440): what the Particle System card's Render section and the `Particles.Set*`
//! render bindings both call, so the two can never clamp differently.

use crate::components::{Flipbook, ParticleEmitterComponent, ParticleRenderMode};

/// Set how the particles are drawn (billboard / stretched / flat / mesh).
pub fn set_render_mode(p: &mut ParticleEmitterComponent, mode: ParticleRenderMode) {
    p.render.mode = mode;
}

/// Set the stretched-billboard length: `length_scale × size + speed_scale × speed`,
/// each factor clamped to `≥ 0`.
pub fn set_stretch(p: &mut ParticleEmitterComponent, length_scale: f32, speed_scale: f32) {
    p.render.length_scale = length_scale.max(0.0);
    p.render.speed_scale = speed_scale.max(0.0);
}

/// Set (or, with an empty string, clear) the mesh-particle mesh: a primitive name
/// or a model asset path.
pub fn set_render_mesh(p: &mut ParticleEmitterComponent, mesh: String) {
    p.render.mesh = (!mesh.is_empty()).then_some(mesh);
}

/// Set (or, with an empty string, clear) the mesh particles' scene material.
pub fn set_render_material(p: &mut ParticleEmitterComponent, material: String) {
    p.render.material = (!material.is_empty()).then_some(material);
}

/// Set the flipbook grid; `columns`/`rows` clamp to `≥ 1`, `cycles` to `≥ 0`.
pub fn set_flipbook(p: &mut ParticleEmitterComponent, flipbook: Flipbook) {
    p.render.flipbook = Flipbook {
        columns: flipbook.columns.max(1),
        rows: flipbook.rows.max(1),
        cycles: flipbook.cycles.max(0.0),
        random_start: flipbook.random_start,
    };
}

/// Set the soft-particle fade distance (world units, `≥ 0`; `0` = hard edges).
pub fn set_soft_distance(p: &mut ParticleEmitterComponent, distance: f32) {
    p.render.soft_distance = distance.max(0.0);
}

/// Set whether the particles are lit by the scene.
pub fn set_lit(p: &mut ParticleEmitterComponent, lit: bool) {
    p.render.lit = lit;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_ops_clamp_and_clear() {
        let mut p = ParticleEmitterComponent::default();
        set_render_mode(&mut p, ParticleRenderMode::Stretched);
        set_stretch(&mut p, -1.0, 0.5);
        set_soft_distance(&mut p, -2.0);
        set_lit(&mut p, true);
        set_flipbook(
            &mut p,
            Flipbook {
                columns: 0,
                rows: 4,
                cycles: -1.0,
                random_start: true,
            },
        );
        set_render_mesh(&mut p, "Cube".into());
        let r = &p.render;
        assert_eq!(r.mode, ParticleRenderMode::Stretched);
        assert_eq!((r.length_scale, r.speed_scale), (0.0, 0.5));
        assert_eq!(r.soft_distance, 0.0);
        assert!(r.lit);
        assert_eq!((r.flipbook.columns, r.flipbook.rows), (1, 4));
        assert_eq!(r.flipbook.cycles, 0.0);
        assert_eq!(r.mesh.as_deref(), Some("Cube"));
        set_render_mesh(&mut p, String::new());
        assert_eq!(p.render.mesh, None);
    }
}
