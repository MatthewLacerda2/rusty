//! src/components/animator/playhead.rs — how a playhead moves and where its
//! motion's length bounds it (#1362): the one [`Bounds`] that advancing, sampling
//! and animation events all read.

/// How one motion's playhead moves this step: playhead units per second of
/// play (`1` for a clip, `1 / blended length` for a tree) and the length it wraps
/// at when looping, or clamps to when not (the clip's duration, `1` for a tree).
/// A negative `wrap` is an unknown length (a clip that doesn't resolve yet): it
/// neither wraps nor clamps. A zero one is a zero-length clip (a single-keyframe
/// pose): it clamps to `0` when not looping, so it ends on its first step (#1386).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playhead {
    pub per_second: f32,
    pub wrap: f32,
}

impl Playhead {
    /// A clip's playhead: seconds, wrapping at `duration`.
    pub fn clip(duration: f32) -> Self {
        Self {
            per_second: 1.0,
            wrap: duration,
        }
    }

    /// A clip that doesn't resolve (yet): seconds, with no known length.
    pub fn unresolved() -> Self {
        Self::clip(-1.0)
    }

    /// A blend tree's playhead: normalized, one cycle is `1`.
    pub fn tree(per_second: f32) -> Self {
        Self {
            per_second,
            wrap: 1.0,
        }
    }

    /// How this playhead treats its length under `loop_clip` — the one decision
    /// advancing, sampling and animation events all read (#1362).
    pub fn bounds(self, loop_clip: bool) -> Bounds {
        match (loop_clip, self.wrap) {
            (true, w) if w > 0.0 => Bounds::Wrap(w),
            (false, w) if w >= 0.0 => Bounds::Clamp(w),
            // An unknown length, or a zero-length loop (nothing to wrap at).
            _ => Bounds::Open,
        }
    }
}

/// How a playhead treats its motion's length: wraps at it (looping), clamps to
/// `[0, length]` (non-looping, #1362), or has none (unknown length).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bounds {
    Wrap(f32),
    Clamp(f32),
    Open,
}

impl Bounds {
    /// `time` within these bounds (a pure `rem_euclid` when wrapping: deterministic).
    pub fn apply(self, time: f32) -> f32 {
        match self {
            Bounds::Wrap(w) => time.rem_euclid(w),
            Bounds::Clamp(end) => time.clamp(0.0, end),
            Bounds::Open => time,
        }
    }
}

/// `x` held within `f32` range, NaN as `0` (a node's `speed: 0` × an infinite one).
pub(super) fn finite(x: f32) -> f32 {
    if x.is_nan() {
        0.0
    } else {
        x.clamp(f32::MIN, f32::MAX)
    }
}
