//! src/components/ribbon/trail.rs — TrailRenderer: a ribbon along the entity's
//! recent path.
//!
//! The recorded points are **sim state**: `app::trails` samples the entity's world
//! position once per fixed tick and ages the points on the same `dt`, so a
//! headless replay records exactly the same trail as a windowed run. The renderer
//! only reads [`TrailRuntime::points`].

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::RibbonStyle;
use crate::core::curve::{Curve, Gradient};

/// One recorded trail point: where the entity was and when (trail clock seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrailPoint {
    pub position: Vec3,
    pub time: f32,
}

/// The live recording — transient, never serialized.
#[derive(Clone, Debug, Default)]
pub struct TrailRuntime {
    /// Oldest first; the last point is the head, which follows the entity.
    pub points: Vec<TrailPoint>,
    /// Seconds this trail has been ticked (the points' timestamps' clock).
    pub clock: f32,
}

/// Authoring component: a trail behind a moving entity (Unity's `TrailRenderer`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrailComponent {
    /// Whether new points are recorded. Off, the existing trail still ages out.
    pub emitting: bool,
    /// Seconds a point lives before it drops off the tail.
    pub time: f32,
    /// The head must move this far (world units) before it is committed and a new
    /// head starts; below it the head just follows the entity.
    pub min_vertex_distance: f32,
    pub style: RibbonStyle,
    #[serde(skip)]
    pub runtime: TrailRuntime,
}

impl Default for TrailComponent {
    fn default() -> Self {
        Self {
            emitting: true,
            time: 0.5,
            min_vertex_distance: 0.1,
            // Tapering from 0.1 to nothing and fading out, as Unity's default.
            style: RibbonStyle {
                width: Curve::linear(0.1, 0.0),
                color: Gradient::fade_out(),
                ..RibbonStyle::default()
            },
            runtime: TrailRuntime::default(),
        }
    }
}

// Runtime state never takes part in equality: two trails with the same settings
// are the same component whatever they have recorded.
impl PartialEq for TrailRuntime {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl TrailComponent {
    /// Advance the trail one tick: age out expired points, then (when emitting)
    /// record `position` — moving the head, or committing it and starting a new
    /// one once it is `min_vertex_distance` past the previous point.
    pub fn advance(&mut self, position: Vec3, dt: f32) {
        let rt = &mut self.runtime;
        rt.clock += dt;
        let oldest = rt.clock - self.time;
        let expired = rt.points.iter().take_while(|p| p.time < oldest).count();
        rt.points.drain(..expired);
        if !self.emitting {
            return;
        }
        let point = TrailPoint {
            position,
            time: rt.clock,
        };
        let n = rt.points.len();
        let anchored =
            n >= 2 && rt.points[n - 2].position.distance(position) < self.min_vertex_distance;
        match rt.points.last_mut() {
            Some(head) if anchored => *head = point,
            _ => rt.points.push(point),
        }
    }

    /// Forget every recorded point (Unity's `TrailRenderer.Clear`).
    pub fn clear(&mut self) {
        self.runtime.points.clear();
    }

    /// The recorded positions, oldest first.
    pub fn positions(&self) -> impl DoubleEndedIterator<Item = Vec3> + '_ {
        self.runtime.points.iter().map(|p| p.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trail() -> TrailComponent {
        TrailComponent {
            time: 1.0,
            min_vertex_distance: 0.5,
            ..Default::default()
        }
    }

    #[test]
    fn head_follows_until_it_passes_min_distance() {
        let mut t = trail();
        t.advance(Vec3::ZERO, 0.1);
        t.advance(Vec3::X * 0.2, 0.1);
        assert_eq!(t.runtime.points.len(), 2, "the second sample starts a head");
        t.advance(Vec3::X * 0.4, 0.1);
        assert_eq!(
            t.runtime.points.len(),
            2,
            "short of min distance: head moves"
        );
        assert_eq!(t.runtime.points[1].position, Vec3::X * 0.4);
        t.advance(Vec3::X * 0.6, 0.1);
        assert_eq!(t.runtime.points.len(), 3, "past it: the head is committed");
    }

    #[test]
    fn points_age_out_and_not_emitting_records_nothing() {
        let mut t = trail();
        for i in 0..5 {
            t.advance(Vec3::X * i as f32, 0.4);
        }
        // Clock 2.0: points stamped before 1.0 are gone.
        assert!(t.runtime.points.iter().all(|p| p.time >= 1.0));
        assert_eq!(t.runtime.points.len(), 3);
        t.emitting = false;
        t.advance(Vec3::X * 9.0, 0.4);
        assert_eq!(t.runtime.points.len(), 2, "aged, nothing recorded");
        t.clear();
        assert!(t.runtime.points.is_empty());
    }
}
