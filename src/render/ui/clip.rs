//! src/render/ui/clip.rs — how a UI graphic is clipped (#418, #428). Pure.
//!
//! Three things clip a graphic, all inherited down the hierarchy walk and all
//! carried per batch as one [`UiClip`]:
//!
//! - **`RectMask` bounds** — every mask on the chain intersected, as pixel bounds.
//!   An overlay canvas also applies them as a hardware [`Scissor`] (cheap pixel
//!   rejection); the shader cuts them again per fragment, which is how a world
//!   canvas clips at all (#619).
//! - **`RectMask.feather`** (#428) — each edge of the intersection remembers the
//!   feather of the mask it came from, so a soft list inside a hard panel fades only
//!   at the list's own edges. The shader fades coverage over that many pixels
//!   inside the edge.
//! - **`Mask`** (#428) — the nearest graphic mask above, by entity id. Its graphic
//!   is rendered into a coverage texture (`effects::mask_gpu`) that already
//!   includes every mask above it, so nesting composes by construction and a batch
//!   samples only one texture. Its rect also joins the bounds, hard.

use glam::{Vec2, Vec4};

use crate::ecs::World;
use crate::ui::UiRect;

/// A scissor rect in target pixels, top-left origin (wgpu's convention).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Scissor {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) w: u32,
    pub(crate) h: u32,
}

impl Scissor {
    /// This rect on a `frame`-pixel target as NDC bounds `[min.x, min.y, max.x,
    /// max.y]` — the space the canvas's vertices are in.
    pub(crate) fn ndc(self, frame: Vec2) -> [f32; 4] {
        let frame = frame.max(Vec2::ONE);
        let lo = Vec2::new(self.x as f32, frame.y - (self.y + self.h) as f32);
        let hi = lo + Vec2::new(self.w as f32, self.h as f32);
        let (lo, hi) = (lo / frame * 2.0 - Vec2::ONE, hi / frame * 2.0 - Vec2::ONE);
        [lo.x, lo.y, hi.x, hi.y]
    }
}

/// How one batch is clipped (see the module docs).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct UiClip {
    /// The rect clip as a scissor, or `None` for the whole target.
    pub(crate) rect: Option<Scissor>,
    /// Each edge's feather in target pixels: left, bottom, right, top.
    pub(crate) feather: [f32; 4],
    /// The nearest `Mask` above, by entity id.
    pub(crate) mask: Option<u32>,
}

impl UiClip {
    /// The shader's view on a `frame`-pixel target: the rect as NDC bounds (or
    /// unbounded) and the feather per edge in NDC units.
    pub(crate) fn ndc(&self, frame: Vec2) -> ([f32; 4], [f32; 4]) {
        let rect = self.rect.map_or(NO_CLIP, |s| s.ndc(frame));
        let unit = 2.0 / frame.max(Vec2::ONE);
        let [l, b, r, t] = self.feather;
        (rect, [l * unit.x, b * unit.y, r * unit.x, t * unit.y])
    }
}

/// NDC bounds that clip nothing (a graphic may overflow its canvas's rect).
pub(crate) const NO_CLIP: [f32; 4] = [-f32::MAX, -f32::MAX, f32::MAX, f32::MAX];

/// Rect-clip bounds in target pixels (bottom-left origin) and each edge's feather
/// (left, bottom, right, top).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SoftRect {
    lo: Vec2,
    hi: Vec2,
    feather: Vec4,
}

impl SoftRect {
    /// The intersection with `other`: each edge is the tighter one, with its feather.
    fn intersect(self, other: SoftRect) -> SoftRect {
        let pick = |mine: f32, theirs: f32, tighter: bool, f: f32, g: f32| match tighter {
            true => (mine, f),
            false => (theirs, g),
        };
        let (a, b) = (self, other);
        let (l, fl) = pick(a.lo.x, b.lo.x, a.lo.x >= b.lo.x, a.feather.x, b.feather.x);
        let (bt, fb) = pick(a.lo.y, b.lo.y, a.lo.y >= b.lo.y, a.feather.y, b.feather.y);
        let (r, fr) = pick(a.hi.x, b.hi.x, a.hi.x <= b.hi.x, a.feather.z, b.feather.z);
        let (t, ft) = pick(a.hi.y, b.hi.y, a.hi.y <= b.hi.y, a.feather.w, b.feather.w);
        SoftRect {
            lo: Vec2::new(l, bt),
            hi: Vec2::new(r, t),
            feather: Vec4::new(fl, fb, fr, ft),
        }
    }
}

/// What an element passes to its children.
#[derive(Clone, Copy, Debug)]
pub(super) struct Inherited {
    pub(super) visible: bool,
    pub(super) alpha: f32,
    clip: Option<SoftRect>,
    mask: Option<u32>,
}

impl Inherited {
    /// A root canvas's starting state: visible only when it and every ancestor are
    /// active.
    pub(super) fn root(world: &World, id: u32) -> Self {
        let mut visible = true;
        let mut cur = Some(id);
        for _ in 0..=world.len() {
            let Some(c) = cur else { break };
            visible &= world.is_active(c);
            cur = world.parent_id(c);
        }
        Self {
            visible,
            alpha: 1.0,
            clip: None,
            mask: None,
        }
    }

    /// Fold `id`'s own active flag, CanvasGroup, RectMask and Mask into this
    /// (its parent's) state: `(what its own graphics draw under, what its children
    /// inherit)`. A RectMask clips its own graphics too; a Mask only its children —
    /// its graphic is what shapes them.
    pub(super) fn child(self, world: &World, id: u32, rect: &UiRect) -> (Self, Self) {
        let mut own = Self {
            visible: self.visible && world.is_active(id),
            alpha: self.alpha * world.canvas_group(id).map_or(1.0, |g| g.alpha),
            ..self
        };
        let bounds = |pad: Vec4, feather: f32| {
            let (lo, hi) = rect.bounds();
            SoftRect {
                lo: (lo + Vec2::new(pad.x, pad.y)) * rect.scale_factor,
                hi: (hi - Vec2::new(pad.z, pad.w)) * rect.scale_factor,
                feather: Vec4::splat(feather * rect.scale_factor),
            }
        };
        if let Some(m) = world.rect_mask(id) {
            own.clip = Some(own.intersect(bounds(m.padding, m.feather)));
        }
        let mut down = own;
        if world.has_mask(id) {
            down.clip = Some(down.intersect(bounds(Vec4::ZERO, 0.0)));
            down.mask = Some(id);
        }
        (own, down)
    }

    fn intersect(self, rect: SoftRect) -> SoftRect {
        self.clip.map_or(rect, |c| c.intersect(rect))
    }

    /// The clip a graphic of opacity `alpha` draws under on a `frame`-pixel target,
    /// or `None` when it draws nothing (hidden, fully transparent, or clipped away
    /// entirely).
    pub(super) fn visible_clip(self, alpha: f32, frame: Vec2) -> Option<UiClip> {
        if !self.visible || alpha <= 0.0 {
            return None;
        }
        self.clip_on(frame)
    }

    /// The clip on a `frame`-pixel target regardless of opacity, or `None` when it
    /// is clipped away entirely.
    pub(super) fn clip_on(self, frame: Vec2) -> Option<UiClip> {
        let Some(soft) = self.clip else {
            return Some(UiClip {
                mask: self.mask,
                ..Default::default()
            });
        };
        Some(UiClip {
            rect: Some(scissor(soft.lo, soft.hi, frame)?),
            feather: soft.feather.to_array(),
            mask: self.mask,
        })
    }
}

/// Pixel bounds (bottom-left origin) → a top-left scissor clamped to the target, or
/// `None` when nothing is left.
fn scissor(lo: Vec2, hi: Vec2, screen: Vec2) -> Option<Scissor> {
    let lo = lo.floor().clamp(Vec2::ZERO, screen);
    let hi = hi.ceil().clamp(Vec2::ZERO, screen);
    if hi.x <= lo.x || hi.y <= lo.y {
        return None;
    }
    Some(Scissor {
        x: lo.x as u32,
        y: (screen.y - hi.y) as u32,
        w: (hi.x - lo.x) as u32,
        h: (hi.y - lo.y) as u32,
    })
}

#[cfg(test)]
#[path = "clip_tests.rs"]
mod clip_tests;
