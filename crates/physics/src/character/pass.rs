use glam::Vec3;
use rapier3d::control::CharacterCollision as Hit;

pub(super) struct Pass {
    pub(super) translation: Vec3,
    pub(super) grounded: bool,
    pub(super) flags: u8,
    pub(super) ground: Option<Vec3>,
    pub(super) collisions: Vec<Hit>,
}

impl Pass {
    pub(super) fn after(mut self, mut earlier: Vec<Hit>) -> Self {
        earlier.append(&mut self.collisions);
        self.collisions = earlier;
        self
    }
}

/// Whether a pass lost over 1% of its horizontal travel. rapier 0.36's controller can on a
/// near-flat floor: its no-slipping rule reads float noise in the downward part as a slip
/// and drops the whole slide. The rerun has no downward part, and snap-to-ground still
/// holds the floor; a wall that took the travel takes it again on the rerun.
pub(super) fn stalled(motion: Vec3, pass: &Pass) -> bool {
    let wanted = Vec3::new(motion.x, 0.0, motion.z).length();
    let got = Vec3::new(pass.translation.x, 0.0, pass.translation.z).length();
    got < wanted * 0.99
}

pub(super) fn off_slope(motion: Vec3, normal: Vec3) -> Vec3 {
    let away = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
    let into = motion.dot(away);
    if into < 0.0 {
        motion - away * into
    } else {
        motion
    }
}
