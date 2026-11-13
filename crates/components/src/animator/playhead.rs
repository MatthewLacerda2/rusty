//! crates/components/src/animator/playhead.rs — how a playhead moves and where its
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

/// What one [`Playback::advance`](super::Playback::advance) did to the current playhead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// The playhead's travel, `(from, to)` before any wrap or clamp — what
    /// animation events (#459) are crossed against. A clamped playhead already
    /// resting on the end it is pushed against travels nothing, so a marker there
    /// fires once per pass, not every step; nor does the step a clip's length
    /// becomes known, since its unresolved run already crossed the markers (#1386).
    pub span: (f32, f32),
    /// The part of `span` whose events fire: all of it, except where a
    /// sync-group follower re-crosses travel it was re-matched back over
    /// (#1896), so a marker fires once, not twice.
    pub events: (f32, f32),
    /// A non-looping playhead reached its end this step, playing forwards or
    /// standing still: once per pass, including the step a clip's length became
    /// known with the playhead already past it, and a zero-length clip's first
    /// (#1386). What fires the automatic `End` event.
    pub end: bool,
}

impl Step {
    pub(super) fn travel(from: f32, to: f32) -> Self {
        Self {
            span: (from, to),
            events: (from, to),
            end: false,
        }
    }
}

/// `delta` taken the shortest way round a loop of length `wrap`, in
/// `[-wrap / 2, wrap / 2)`: how a sync-group follower (#1896) moves to a
/// re-matched phase.
pub fn wrap_delta(delta: f32, wrap: f32) -> f32 {
    (delta + wrap * 0.5).rem_euclid(wrap) - wrap * 0.5
}

/// `x` held within `f32` range, NaN as `0` (a node's `speed: 0` × an infinite one).
pub(super) fn finite(x: f32) -> f32 {
    if x.is_nan() {
        0.0
    } else {
        x.clamp(f32::MIN, f32::MAX)
    }
}
