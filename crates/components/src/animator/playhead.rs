/// Playhead units per second of play, and the length `wrap` (negative: unknown).
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

    pub fn bounds(self, loop_clip: bool) -> Bounds {
        match (loop_clip, self.wrap) {
            (true, w) if w > 0.0 => Bounds::Wrap(w),
            (false, w) if w >= 0.0 => Bounds::Clamp(w),
            _ => Bounds::Open,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bounds {
    Wrap(f32),
    Clamp(f32),
    Open,
}

impl Bounds {
    pub fn apply(self, time: f32) -> f32 {
        match self {
            Bounds::Wrap(w) => time.rem_euclid(w),
            Bounds::Clamp(end) => time.clamp(0.0, end),
            Bounds::Open => time,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// Travel before wrap or clamp, which events are crossed against.
    pub span: (f32, f32),
    /// The part of `span` whose events fire (not a sync follower's re-crossed travel).
    pub events: (f32, f32),
    /// Reached a non-looping end, once per pass: what fires the automatic `End`.
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

/// `delta` the shortest way round a loop of `wrap`, in `[-wrap / 2, wrap / 2)`.
pub fn wrap_delta(delta: f32, wrap: f32) -> f32 {
    (delta + wrap * 0.5).rem_euclid(wrap) - wrap * 0.5
}

/// NaN as 0 (`speed: 0` × an infinite one), else within `f32` range.
pub(super) fn finite(x: f32) -> f32 {
    if x.is_nan() {
        0.0
    } else {
        x.clamp(f32::MIN, f32::MAX)
    }
}
