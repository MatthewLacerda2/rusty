//! src/components/particle/legacy.rs — loading pre-#439 emitters.
//!
//! Before #439 an emitter lerped its size from `size_start` to `size_end` over
//! life. The start size now loads straight into `size` (a serde alias), but the
//! end size has no field of its own any more: it becomes the size-over-life curve.
//! `EntityRepr` (the scene's legacy-tolerant load shape) reads emitters through
//! this wrapper, so the migration lives here instead of on the component.

use serde::Deserialize;

use super::ParticleEmitterComponent;
use crate::core::curve::{Curve, Range};

/// An emitter as stored on disk, with the retired `size_end` key.
#[derive(Deserialize)]
pub struct LegacyEmitter {
    #[serde(flatten)]
    emitter: ParticleEmitterComponent,
    #[serde(default)]
    size_end: Option<f32>,
}

impl From<LegacyEmitter> for ParticleEmitterComponent {
    fn from(l: LegacyEmitter) -> Self {
        let mut e = l.emitter;
        let Some(end) = l.size_end else {
            return e;
        };
        let start = e.size.min;
        if start > 0.0 {
            e.size_over_life = Curve::linear(1.0, end / start);
        } else {
            // A zero start can't be scaled up: grow from nothing to `end` instead.
            e.size = Range::constant(end);
            e.size_over_life = Curve::linear(0.0, 1.0);
        }
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_end_becomes_the_size_curve() {
        let json = r#"{"active":true,"emit_mode":"Continuous","blend":"Alpha",
            "collision":"None","rate":20.0,"burst_count":32,"max_particles":256,
            "looping":true,"lifetime":1.5,"speed":2.0,"direction":[0.0,1.0,0.0],
            "spread":0.2,"gravity":[0.0,0.0,0.0],"size_start":0.5,"size_end":1.5,
            "color":[1.0,1.0,1.0,1.0],"bounciness":0.5,"seed":1}"#;
        let l: LegacyEmitter = serde_json::from_str(json).unwrap();
        let e = ParticleEmitterComponent::from(l);
        assert_eq!(e.size, Range::constant(0.5));
        assert_eq!(e.lifetime, Range::constant(1.5));
        assert_eq!(e.size_over_life.evaluate(1.0), 3.0);
        assert_eq!(e.color_over_life.evaluate(1.0)[3], 0.0, "alpha still fades");
    }
}
