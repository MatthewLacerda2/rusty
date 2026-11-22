use std::f32::consts::PI;

use glam::{Vec3, Vec4};

use super::super::rng::Rng;
use super::sources::Picks;
use super::{TexelLight, Tracer, SURFACE_OFFSET};

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::lightmap) struct Paths {
    pub sum: Vec3,
    pub dir: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::lightmap) struct Shown {
    pub direct: Vec3,
    pub direct_dir: Vec3,
    pub mask: [f32; 4],
}

impl Paths {
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
    pub(in crate::lightmap) fn texel(
        &self,
        position: Vec3,
        normal: Vec3,
        (samples, direct): (u32, u32),
        bounces: u32,
        rng: &mut Rng,
    ) -> TexelLight {
        let mut paths = Paths::default();
        self.gather(position, normal, (samples, bounces), rng, &mut paths);
        self.resolve(position, normal, (samples, direct), &paths, rng)
    }

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

    pub(in crate::lightmap) fn resolve(
        &self,
        position: Vec3,
        normal: Vec3,
        (samples, direct): (u32, u32),
        paths: &Paths,
        rng: &mut Rng,
    ) -> TexelLight {
        let shown = self.shown(position, normal, direct, rng);
        paths.light(samples, &shown)
    }

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
