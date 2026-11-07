//! Occluder fusion on a plane (#1714, part 3): the occluder faces that lie on one
//! plane and draw the same side, as one sheet. A floor of tiles, a wall built of
//! pieces, the tops of a row of walls and the slab above them each hide, together,
//! what none of their faces hides alone.
//!
//! A sheet is a grid of square texels on its plane, one bit each: set when the
//! texel lies wholly inside one member face. Every segment from one box to
//! another that clear the plane on opposite sides crosses it inside the
//! rectangle round the crossings of their corners' segments (`shaft` says why),
//! so when every texel that rectangle touches is set, every segment crosses a
//! member face, and the box is hidden. A texel half-covered by two faces is not
//! set, so a seam off the texel grid fuses nothing: it only shows more. The grid
//! sits on multiples of [`TEXEL`] in world units, where modular levels put their
//! seams.
//!
//! The members' faces may differ from the plane by a rounding: the sheet is a slab
//! from the nearest to the farthest member corner, and the rectangle holds the
//! crossings of both its planes. An object's own faces never hide it, so a sheet
//! it is a member of is not used for it (`shaft` falls back to its own polygon).

use glam::{UVec2, Vec2, Vec3};

use crate::scene::lighting::occlusion::gather::Face;

/// A texel's edge, metres.
const TEXEL: f32 = 0.25;
/// The most texels one sheet holds; a larger plane takes coarser texels.
const MAX_TEXELS: usize = 1 << 22;
/// How far, metres, both boxes must clear the slab, and a member face's edge may
/// miss a texel's corner and still hold it.
const CLEAR: f32 = 1e-3;
const EDGE: f32 = 1e-4;

/// One plane's fused occluder faces.
pub(super) struct Sheet {
    normal: Vec3,
    /// The slab the members lie in: `lo <= normal · p <= hi`.
    lo: f32,
    hi: f32,
    face: Face,
    /// The plane's axes, and texel `(0, 0)`'s low corner on them.
    axes: (Vec3, Vec3),
    origin: Vec2,
    texel: f32,
    dims: UVec2,
    /// Set texels counted over each prefix rectangle, `(dims + 1)` a side.
    sums: Vec<u32>,
    /// The objects whose faces it holds, ascending.
    members: Vec<u32>,
}

/// One member face: its corners (convex, wound about the sheet's normal or
/// against it), its object and its side.
pub(super) struct Member<'a> {
    pub corners: &'a [Vec3],
    pub normal: Vec3,
    pub mesh: u32,
    pub face: Face,
}

/// The sheets `faces` make, and per face its sheet (`u32::MAX` for a face alone
/// on its plane, which its own polygon tests as well, on a sparse sheet, or with
/// no plane).
pub(super) fn sheets(faces: &[Member]) -> (Vec<Sheet>, Vec<u32>) {
    // Each face keyed by its plane and side, sorted: a plane's faces then run
    // together, in face order.
    let key = |f: &Member| {
        let n = (f.normal * 1e6).round().as_i64vec3().to_array();
        (
            n,
            (f.normal.dot(f.corners[0]) / CLEAR).round() as i64,
            f.face as u8,
        )
    };
    let planar = faces
        .iter()
        .enumerate()
        .filter(|(_, f)| f.normal != Vec3::ZERO);
    let mut keyed: Vec<_> = planar.map(|(i, f)| (key(f), i as u32)).collect();
    keyed.sort_unstable();
    let mut of = vec![u32::MAX; faces.len()];
    let mut sheets = Vec::new();
    for run in keyed.chunk_by(|a, b| a.0 == b.0).filter(|r| r.len() > 1) {
        let sheet = Sheet::of(run.iter().map(|&(_, i)| &faces[i as usize]));
        let Some(sheet) = sheet.filter(|s| !s.sparse()) else {
            continue;
        };
        for &(_, i) in run {
            of[i as usize] = sheets.len() as u32;
        }
        sheets.push(sheet);
    }
    (sheets, of)
}

/// A box's corners, one when they meet.
fn corners((min, max): (Vec3, Vec3)) -> ([Vec3; 8], usize) {
    let pick = |k: usize, bit: usize, axis: usize| if k & bit == 0 { min[axis] } else { max[axis] };
    let all = std::array::from_fn(|k| Vec3::new(pick(k, 1, 0), pick(k, 2, 1), pick(k, 4, 2)));
    (all, if min == max { 1 } else { 8 })
}

impl Sheet {
    /// The sheet of `members`, `None` when their faces cover under a quarter of
    /// the rectangle round them (it would be [`Self::sparse`]: no texel is made).
    fn of<'a>(members: impl Iterator<Item = &'a Member<'a>> + Clone) -> Option<Self> {
        let first = members.clone().next().expect("a sheet has a member");
        let normal = first.normal;
        let axes = axes(normal);
        let flat = |p: Vec3| Vec2::new(axes.0.dot(p), axes.1.dot(p));
        let all = members.clone().flat_map(|m| m.corners.iter().copied());
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for p in all {
            (lo, hi) = (lo.min(normal.dot(p)), hi.max(normal.dot(p)));
            (min, max) = (min.min(flat(p)), max.max(flat(p)));
        }
        let polys: Vec<Vec<Vec2>> = (members.clone())
            .map(|m| m.corners.iter().map(|&p| flat(p)).collect())
            .collect();
        let area: f32 = polys.iter().map(|p| area(p).abs()).sum();
        if area * 4.0 < (max - min).x * (max - min).y {
            return None;
        }
        let mut texel = TEXEL;
        let (origin, dims) = loop {
            let origin = (min / texel).floor() * texel;
            let dims = ((max - origin) / texel).ceil().max(Vec2::ONE).as_uvec2();
            if (dims.x as usize).saturating_mul(dims.y as usize) <= MAX_TEXELS {
                break (origin, dims);
            }
            texel *= 2.0;
        };
        let mut set = vec![false; (dims.x * dims.y) as usize];
        for poly in &polys {
            fill(&mut set, poly, (origin, texel, dims));
        }
        let mut members: Vec<u32> = members.map(|m| m.mesh).collect();
        members.sort_unstable();
        members.dedup();
        Some(Self {
            normal,
            lo,
            hi,
            face: first.face,
            axes,
            origin,
            texel,
            dims,
            sums: sums(&set, dims),
            members,
        })
    }

    /// Whether under a quarter of its texels are set: faces scattered over a plane
    /// (the same facet of props in a row) fuse little, and would cost their
    /// texels' memory for it.
    fn sparse(&self) -> bool {
        let total = self.dims.x as usize * self.dims.y as usize;
        let set = *self.sums.last().unwrap_or(&0) as usize;
        set * 4 < total
    }

    /// The bytes its texels' sums take.
    #[cfg(test)]
    pub fn bytes(&self) -> usize {
        self.sums.len() * 4
    }

    /// Whether `object` has a face on it.
    pub fn holds(&self, object: u32) -> bool {
        self.members.binary_search(&object).is_ok()
    }

    /// Whether every segment from box `from` to box `to` crosses a member face's
    /// drawn side.
    pub fn hides(&self, from: (Vec3, Vec3), to: (Vec3, Vec3)) -> bool {
        let n = self.normal;
        let centre = |b: (Vec3, Vec3)| n.dot((b.0 + b.1) * 0.5);
        let reach = |b: (Vec3, Vec3)| n.abs().dot((b.1 - b.0) * 0.5);
        let (cf, ct) = (centre(from), centre(to));
        if !self.face.stops(n, n * (ct - cf)) {
            return false;
        }
        let below = cf + reach(from) < self.lo - CLEAR && ct - reach(to) > self.hi + CLEAR;
        let above = cf - reach(from) > self.hi + CLEAR && ct + reach(to) < self.lo - CLEAR;
        if !(below || above) {
            return false;
        }
        let ((near, k), (far, j)) = (corners(from), corners(to));
        let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for &a in &near[..k] {
            for &b in &far[..j] {
                let (da, db) = (n.dot(a), n.dot(b));
                for plane in [self.lo, self.hi] {
                    let x = a + (b - a) * ((plane - da) / (db - da));
                    let at = Vec2::new(self.axes.0.dot(x), self.axes.1.dot(x));
                    (min, max) = (min.min(at), max.max(at));
                }
            }
        }
        self.covered(min - Vec2::splat(CLEAR), max + Vec2::splat(CLEAR))
    }

    /// Whether every texel the rectangle `(min, max)` touches is set.
    fn covered(&self, min: Vec2, max: Vec2) -> bool {
        let lo = ((min - self.origin) / self.texel).floor();
        let hi = ((max - self.origin) / self.texel).floor();
        let inside = lo.cmpge(Vec2::ZERO).all() && hi.cmplt(self.dims.as_vec2()).all();
        if !inside || !lo.is_finite() || !hi.is_finite() {
            return false;
        }
        let (lo, hi) = (lo.as_uvec2(), hi.as_uvec2() + UVec2::ONE);
        let w = self.dims.x as usize + 1;
        let at = |p: UVec2| self.sums[p.y as usize * w + p.x as usize] as i64;
        let count = at(hi) - at(UVec2::new(lo.x, hi.y)) - at(UVec2::new(hi.x, lo.y)) + at(lo);
        count == i64::from((hi.x - lo.x) * (hi.y - lo.y))
    }
}

/// Two unit axes on the plane with `normal`: the world axes for an axis-aligned
/// plane, so its texels sit on world multiples.
fn axes(normal: Vec3) -> (Vec3, Vec3) {
    let a = [Vec3::X, Vec3::Y, Vec3::Z];
    let pick = a
        .into_iter()
        .min_by(|p, q| normal.dot(*p).abs().total_cmp(&normal.dot(*q).abs()));
    let a = pick.unwrap_or(Vec3::X);
    let u = (a - normal * normal.dot(a)).normalize_or_zero();
    (u, normal.cross(u))
}

/// Set every texel of the grid `(origin, texel, dims)` wholly inside the convex
/// polygon `poly`.
fn fill(set: &mut [bool], poly: &[Vec2], (origin, texel, dims): (Vec2, f32, UVec2)) {
    let k = poly.len();
    if k < 3 {
        return;
    }
    let turn = area(poly).signum();
    let inside = |p: Vec2| {
        (0..k).all(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % k]);
            turn * (b - a).perp_dot(p - a) >= -EDGE * (b - a).length()
        })
    };
    let (min, max) = poly.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), &p| {
        (lo.min(p), hi.max(p))
    });
    let first = ((min - origin) / texel).floor().max(Vec2::ZERO).as_uvec2();
    let last = ((max - origin) / texel).ceil().as_uvec2().min(dims);
    for y in first.y..last.y {
        for x in first.x..last.x {
            let lo = origin + UVec2::new(x, y).as_vec2() * texel;
            let square = [
                lo,
                lo + Vec2::X * texel,
                lo + Vec2::Y * texel,
                lo + Vec2::splat(texel),
            ];
            if square.into_iter().all(inside) {
                set[(y * dims.x + x) as usize] = true;
            }
        }
    }
}

/// The signed area of `poly`, positive when it winds counter-clockwise.
fn area(poly: &[Vec2]) -> f32 {
    let k = poly.len();
    (0..k)
        .map(|i| poly[i].perp_dot(poly[(i + 1) % k]))
        .sum::<f32>()
        * 0.5
}

/// The prefix sums of `set` over a `dims` grid, `(dims + 1)` a side.
fn sums(set: &[bool], dims: UVec2) -> Vec<u32> {
    let w = dims.x as usize + 1;
    let mut s = vec![0u32; w * (dims.y as usize + 1)];
    for y in 0..dims.y as usize {
        for x in 0..dims.x as usize {
            let v = u32::from(set[y * dims.x as usize + x]);
            s[(y + 1) * w + x + 1] = v + s[y * w + x + 1] + s[(y + 1) * w + x] - s[y * w + x];
        }
    }
    s
}
