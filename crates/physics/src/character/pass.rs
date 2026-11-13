//! crates/physics/src/character/pass.rs — one run of rapier's controller inside a
//! `Move` ([`Pass`]), and the two corrections `sweep` reruns it for: a floor
//! that ate the walk ([`stalled`]) and a push into a steep slope ([`off_slope`]).

use glam::Vec3;
use rapier3d::control::CharacterCollision as Hit;

/// What one run of the controller produced.
pub(super) struct Pass {
    pub(super) translation: Vec3,
    pub(super) grounded: bool,
    pub(super) flags: u8,
    /// The normal of the last floor the sweep hit below the capsule.
    pub(super) ground: Option<Vec3>,
    /// Every contact of the sweep, in the order it met them.
    pub(super) collisions: Vec<Hit>,
}

impl Pass {
    /// This rerun, with what the discarded pass before it touched kept ahead
    /// of its own contacts (`controller_hits` folds the repeats): a steep
    /// slope the first pass met, or a floor, still counts as hit.
    pub(super) fn after(mut self, mut earlier: Vec<Hit>) -> Self {
        earlier.append(&mut self.collisions);
        self.collisions = earlier;
        self
    }
}

/// Whether a pass lost horizontal travel. A floor never blocks walking, but
/// rapier 0.36's controller can: against a near-flat floor its "no slipping on
/// a walkable slope" rule reads float noise in the downward (gravity) part as a
/// slip and drops the whole slide, stalling the character for a tick, on open
/// ground and sliding along a wall alike. Without the downward part there is
/// nothing to misread, and snap-to-ground still keeps the capsule on the floor.
/// A wall that took the travel takes it again on the rerun, so the rerun only
/// ever gives back what the floor ate.
pub(super) fn stalled(motion: Vec3, pass: &Pass) -> bool {
    let wanted = Vec3::new(motion.x, 0.0, motion.z).length();
    let got = Vec3::new(pass.translation.x, 0.0, pass.translation.z).length();
    got < wanted * 0.99
}

/// `motion` without its horizontal push into the steep surface `normal`.
pub(super) fn off_slope(motion: Vec3, normal: Vec3) -> Vec3 {
    let away = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
    let into = motion.dot(away);
    if into < 0.0 {
        motion - away * into
    } else {
        motion
    }
}
