use glam::{Vec2, Vec4};

use rusty_ecs::World;
use rusty_ui::UiRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Scissor {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) w: u32,
    pub(crate) h: u32,
}

impl Scissor {
    pub(crate) fn ndc(self, frame: Vec2) -> [f32; 4] {
        let frame = frame.max(Vec2::ONE);
        let lo = Vec2::new(self.x as f32, frame.y - (self.y + self.h) as f32);
        let hi = lo + Vec2::new(self.w as f32, self.h as f32);
        let (lo, hi) = (lo / frame * 2.0 - Vec2::ONE, hi / frame * 2.0 - Vec2::ONE);
        [lo.x, lo.y, hi.x, hi.y]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct UiClip {
    pub(crate) rect: Option<Scissor>,
    pub(crate) feather: [f32; 4],
    pub(crate) mask: Option<u32>,
}

impl UiClip {
    pub(crate) fn ndc(&self, frame: Vec2) -> ([f32; 4], [f32; 4]) {
        let rect = self.rect.map_or(NO_CLIP, |s| s.ndc(frame));
        let unit = 2.0 / frame.max(Vec2::ONE);
        let [l, b, r, t] = self.feather;
        (rect, [l * unit.x, b * unit.y, r * unit.x, t * unit.y])
    }
}

pub(crate) const NO_CLIP: [f32; 4] = [-f32::MAX, -f32::MAX, f32::MAX, f32::MAX];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SoftRect {
    lo: Vec2,
    hi: Vec2,
    feather: Vec4,
}

impl SoftRect {
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

#[derive(Clone, Copy, Debug)]
pub(super) struct Inherited {
    pub(super) visible: bool,
    pub(super) alpha: f32,
    clip: Option<SoftRect>,
    mask: Option<u32>,
}

impl Inherited {
    pub(super) fn root(world: &World, id: u32) -> Self {
        Self {
            visible: world.is_active_in_hierarchy(id),
            alpha: 1.0,
            clip: None,
            mask: None,
        }
    }

    pub(super) fn child(self, world: &World, id: u32, rect: &UiRect) -> (Self, Self) {
        let mut own = Self {
            // lint: allow(active-self) folds one level of the walk `root` began
            visible: self.visible && world.is_active_self(id),
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

    pub(super) fn visible_clip(self, alpha: f32, frame: Vec2) -> Option<UiClip> {
        if !self.visible || alpha <= 0.0 {
            return None;
        }
        self.clip_on(frame)
    }

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
