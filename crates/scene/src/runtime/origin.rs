//! How far Play has shifted the World from authored space: `authored = world + offset()`.
//! Readers outside the sim follow each shift by [`FloatingOrigin::serial`].

use glam::{DVec3, IVec3, Vec3};

pub use rusty_core::settings::origin::{OriginSettings, ORIGIN_STEP};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FloatingOrigin {
    pub settings: OriginSettings,
    /// `None` follows the main camera.
    pub focus: Option<u32>,
    cells: IVec3,
    serial: u32,
    last: IVec3,
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

    pub fn serial(&self) -> u32 {
        self.serial
    }

    /// What every World position moved by in the latest shift.
    pub fn last_shift(&self) -> Vec3 {
        (-self.last).as_vec3() * ORIGIN_STEP
    }

    /// Steps per far axis, rounded: the focus lands within half a step of zero.
    pub fn plan(&self, at: Vec3) -> Option<IVec3> {
        if !self.settings.enabled || !at.is_finite() {
            return None;
        }
        let far = at.abs().cmpgt(Vec3::splat(self.settings.threshold));
        if !far.any() {
            return None;
        }
        let steps = (at / ORIGIN_STEP).round().as_ivec3();
        let steps = IVec3::select(far, steps, IVec3::ZERO);
        (steps != IVec3::ZERO).then_some(steps)
    }

    pub fn shift(&mut self, steps: IVec3) -> Vec3 {
        self.cells += steps;
        self.serial = self.serial.wrapping_add(1);
        self.last = steps;
        self.last_shift()
    }

    /// The setting and serial stay.
    pub fn reset(&mut self) {
        self.cells = IVec3::ZERO;
        self.last = IVec3::ZERO;
        self.focus = None;
    }

    pub fn steps(&self) -> IVec3 {
        self.cells
    }
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod origin_tests;
