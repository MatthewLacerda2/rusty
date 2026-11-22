use rusty_components::{Flipbook, ParticleEmitterComponent, ParticleRenderMode, TaaReactive};
use rusty_scene::rules::sanitize::non_negative;

pub fn set_render_mode(p: &mut ParticleEmitterComponent, mode: ParticleRenderMode) {
    p.render.mode = mode;
    super::Dishonoured::Mesh.warn(p);
}

/// Set the stretched-billboard length: `length_scale × size + speed_scale × speed`,
/// each factor clamped to `≥ 0`.
pub fn set_stretch(p: &mut ParticleEmitterComponent, length_scale: f32, speed_scale: f32) {
    p.render.length_scale = non_negative(length_scale);
    p.render.speed_scale = non_negative(speed_scale);
}

pub fn set_render_mesh(p: &mut ParticleEmitterComponent, mesh: String) {
    p.render.mesh = (!mesh.is_empty()).then_some(mesh);
}

pub fn set_render_material(p: &mut ParticleEmitterComponent, material: String) {
    p.render.material = (!material.is_empty()).then_some(material);
}

// lint: allow(sanitize) its one float, `cycles`, goes through `non_negative`
pub fn set_flipbook(p: &mut ParticleEmitterComponent, flipbook: Flipbook) {
    p.render.flipbook = Flipbook {
        columns: flipbook.columns.max(1),
        rows: flipbook.rows.max(1),
        cycles: non_negative(flipbook.cycles),
        random_start: flipbook.random_start,
    };
}

pub fn set_soft_distance(p: &mut ParticleEmitterComponent, distance: f32) {
    p.render.soft_distance = non_negative(distance);
}

pub fn set_reactive(p: &mut ParticleEmitterComponent, reactive: TaaReactive) {
    p.render.reactive = reactive;
}

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
