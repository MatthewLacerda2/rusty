//! One texel's light traced in parts (#1970): its bounce paths summed as they go
//! ([`Paths`]), then resolved into a [`TexelLight`]. The one-shot bake traces every
//! path at once ([`Tracer::texel`]); a progressive bake traces them a pass at a
//! time, keeping each texel's [`Paths`] and its seeded stream between passes, and
//! resolves at the end. The sums are added in the same order either way, and the
//! texel's direct light is drawn from its stream after its last path either way,
//! so the two give the same bits.

use std::f32::consts::PI;

use glam::{Vec3, Vec4};

use super::super::rng::Rng;
use super::sources::Picks;
use super::{TexelLight, Tracer, SURFACE_OFFSET};

/// A texel's bounce paths so far: the radiance they brought back and its
/// luminance-weighted direction, summed (never averaged, so a pass boundary never
/// rounds them).
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::lightmap) struct Paths {
    pub sum: Vec3,
    pub dir: Vec3,
}

/// What a progressive bake shows of a texel before it is finished: its direct
/// light and shadowmask, taken once from a display stream of its own
/// (`Rng::preview`), so its own stream stays as the one-shot bake draws it.
#[derive(Clone, Copy, Debug)]
pub(in crate::lightmap) struct Shown {
    pub direct: Vec3,
    pub direct_dir: Vec3,
    /// A `Vec4`'s floats, unaligned: a progressive bake holds one per texel
    /// between passes, so it stays 40 bytes, not 48.
    pub mask: [f32; 4],
}

impl Paths {
    /// The texel's light after `done` paths, its direct part from `shown`: the
    /// light a progressive bake displays between passes.
    pub fn light(&self, done: u32, shown: &Shown) -> TexelLight {
        let n = done.max(1) as f32;
        TexelLight {
            indirect: self.sum / n,
            direct: shown.direct,
            indirect_dir: self.dir / n,
            direct_dir: shown.direct_dir,
            mask: Vec4::from_array(shown.mask),
            valid: true,
        }
    }
}

impl Tracer<'_> {
    /// The lightmap value at `position` facing `normal`, in its two parts: the bounce
    /// and sky over `samples` paths `bounces` deep, and the direct light of `Baked`
    /// lights. The bake filters the first and adds the second.
    pub(in crate::lightmap) fn texel(
        &self,
        position: Vec3,
        normal: Vec3,
        samples: u32,
        bounces: u32,
        rng: &mut Rng,
    ) -> TexelLight {
        let mut paths = Paths::default();
        self.gather(position, normal, (samples, bounces), rng, &mut paths);
        self.resolve(position, normal, samples, &paths, rng)
    }

    /// `count` more bounce paths, `bounces` deep, from the texel at `position`
    /// facing `normal`, added to `paths` from `rng`.
    pub(in crate::lightmap) fn gather(
        &self,
        position: Vec3,
        normal: Vec3,
        (count, bounces): (u32, u32),
        rng: &mut Rng,
        paths: &mut Paths,
    ) {
        let origin = position + normal * SURFACE_OFFSET;
        let mut picks = Picks::default();
        for _ in 0..count {
            let (radiance, dir) = self.path(origin, normal, bounces, rng, &mut picks);
            paths.sum += radiance;
            paths.dir += dir;
        }
    }

    /// The texel's light once all `samples` of its `paths` are in: the mean of the
    /// bounce, and its direct light drawn from `rng` (where the paths left it).
    pub(in crate::lightmap) fn resolve(
        &self,
        position: Vec3,
        normal: Vec3,
        samples: u32,
        paths: &Paths,
        rng: &mut Rng,
    ) -> TexelLight {
        let shown = self.shown(position, normal, samples, rng);
        paths.light(samples, &shown)
    }

    /// The texel's direct light (an Area light over `area_samples` or more points
    /// drawn from `rng`) and shadowmask.
    pub(in crate::lightmap) fn shown(
        &self,
        position: Vec3,
        normal: Vec3,
        area_samples: u32,
        rng: &mut Rng,
    ) -> Shown {
        let origin = position + normal * SURFACE_OFFSET;
        let (direct, direct_dir) = self.direct(origin, normal, area_samples, rng);
        Shown {
            direct: direct / PI,
            direct_dir: direct_dir / PI,
            mask: self.mask(origin).to_array(),
        }
    }
}
