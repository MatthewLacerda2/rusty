//! How far Play has shifted the World from authored space: `authored = world + offset()`.
//! It lives on `LoadedScenes`, the scenes whose authored space it is, and goes with them on
//! Stop.

use glam::{DVec3, IVec3, Vec3};

pub use rusty_core::settings::origin::{OriginSettings, ORIGIN_STEP};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FloatingOrigin {
    /// `None` follows the main camera.
    pub focus: Option<u32>,
    cells: IVec3,
}

impl FloatingOrigin {
    pub fn offset(&self) -> DVec3 {
        self.cells.as_dvec3() * f64::from(ORIGIN_STEP)
    }

    pub fn offset_f32(&self) -> Vec3 {
        self.cells.as_vec3() * ORIGIN_STEP
    }

    pub fn is_zero(&self) -> bool {
        self.cells == IVec3::ZERO
    }

    /// Steps per far horizontal axis, rounded: the focus lands within half a step of zero.
    /// Never vertical: heights (fog, a sea level) stay authored.
    pub fn plan(&self, at: Vec3, settings: &OriginSettings) -> Option<IVec3> {
        if !settings.enabled || !at.is_finite() {
            return None;
        }
        let far = (at * Vec3::new(1.0, 0.0, 1.0)).abs();
        let far = far.cmpgt(Vec3::splat(settings.threshold));
        if !far.any() {
            return None;
        }
        let steps = (at / ORIGIN_STEP).round().as_ivec3();
        let steps = IVec3::select(far, steps, IVec3::ZERO);
        (steps != IVec3::ZERO).then_some(steps)
    }

    /// Returns what every World position moves by.
    pub fn shift(&mut self, steps: IVec3) -> Vec3 {
        self.cells += steps;
        (-steps).as_vec3() * ORIGIN_STEP
    }

    pub fn steps(&self) -> IVec3 {
        self.cells
    }
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod origin_tests;
