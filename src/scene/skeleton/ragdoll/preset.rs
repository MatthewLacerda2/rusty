//! The humanoid joint preset (#466): which joint each bone gets, keyed by name.
//!
//! Mixamo naming first (`mixamorig:LeftForeArm`): a namespace before the last `:`
//! and a `Left` / `Right` prefix are ignored, and the rest is matched without
//! regard to case. A bone the table doesn't name gets a moderate ball joint, so
//! any rig ragdolls, and the result is ordinary `Joint`s to tweak.
//!
//! Limits follow Unity's Ragdoll Wizard: each joint's **axis** is the one its
//! bone flexes about, so the asymmetric range (a knee bends one way) is the
//! twist range; swing 1 is about the bend direction (a spine bending sideways, an
//! arm raised), swing 2 about the bone itself (its twist). Angles are degrees;
//! a positive flex bends the bone toward [`Bend`].

use crate::components::JointKind;

/// Which way a bone flexes, in the character's own space (glTF: +Y up, facing +Z).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Bend {
    Forward,
    Backward,
}

/// One bone's joint shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct JointPreset {
    pub kind: JointKind,
    pub bend: Bend,
    /// The flex range `(min, max)`, degrees toward `bend`.
    pub flex: (f32, f32),
    /// Swing about the bend direction (Unity's swing 1).
    pub swing1: f32,
    /// Swing about the bone (swing 2).
    pub swing2: f32,
}

const fn ball(flex: (f32, f32), swing1: f32, swing2: f32) -> JointPreset {
    JointPreset {
        kind: JointKind::Ball,
        bend: Bend::Forward,
        flex,
        swing1,
        swing2,
    }
}

const fn hinge(bend: Bend, max: f32) -> JointPreset {
    JointPreset {
        kind: JointKind::Hinge,
        bend,
        flex: (0.0, max),
        swing1: 0.0,
        swing2: 0.0,
    }
}

/// A bone the table doesn't name.
pub(super) const DEFAULT: JointPreset = ball((-30.0, 30.0), 30.0, 30.0);

/// Mixamo bone names, side prefix removed.
const HUMANOID: [(&str, JointPreset); 13] = [
    ("Spine", ball((-20.0, 30.0), 15.0, 15.0)),
    ("Spine1", ball((-20.0, 30.0), 15.0, 15.0)),
    ("Spine2", ball((-20.0, 30.0), 15.0, 15.0)),
    ("Neck", ball((-30.0, 40.0), 20.0, 30.0)),
    ("Head", ball((-30.0, 40.0), 20.0, 40.0)),
    ("Shoulder", ball((-10.0, 15.0), 10.0, 10.0)),
    ("Arm", ball((-50.0, 90.0), 70.0, 50.0)),
    ("ForeArm", hinge(Bend::Forward, 140.0)),
    ("Hand", ball((-50.0, 50.0), 30.0, 20.0)),
    ("UpLeg", ball((-20.0, 100.0), 35.0, 20.0)),
    ("Leg", hinge(Bend::Backward, 130.0)),
    ("Foot", ball((-30.0, 30.0), 15.0, 15.0)),
    ("ToeBase", ball((-20.0, 20.0), 5.0, 5.0)),
];

/// The joint for a bone named `name`.
pub(super) fn preset_for(name: &str) -> JointPreset {
    let bare = name.rsplit(':').next().unwrap_or(name);
    let bare = strip_side(bare);
    HUMANOID
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(bare))
        .map_or(DEFAULT, |&(_, p)| p)
}

/// `name` without a leading `Left` / `Right` (any case).
fn strip_side(name: &str) -> &str {
    for side in ["left", "right"] {
        match (name.get(..side.len()), name.get(side.len()..)) {
            (Some(head), Some(rest)) if !rest.is_empty() && head.eq_ignore_ascii_case(side) => {
                return rest;
            }
            _ => {}
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixamo_names_match_whatever_the_namespace_side_or_case() {
        assert_eq!(
            preset_for("mixamorig:LeftForeArm"),
            hinge(Bend::Forward, 140.0)
        );
        assert_eq!(preset_for("RightLeg"), hinge(Bend::Backward, 130.0));
        assert_eq!(preset_for("mixamorig9:spine2").flex, (-20.0, 30.0));
        assert_eq!(preset_for("Head").swing2, 40.0);
        assert_eq!(preset_for("tail_03"), DEFAULT);
        assert_eq!(preset_for("Left"), DEFAULT, "a bare side is not a bone");
    }
}
