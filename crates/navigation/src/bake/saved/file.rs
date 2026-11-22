//! Little-endian: `RNAV`, format `u32`, fingerprint `u64`, grid (min/max x, min/max z, spacing
//! `f32`; width, height `u32`), baked then pre-erosion spans (`u32` cell-start count and starts,
//! `u32` span count; each span floor, ceiling `f32`, area `u8`, surface kind `u8` followed by a
//! level plane's sign bits or [`PLANE`] and three `f32`s, floor `f32`), then the off-mesh links. A
//! truncated, out-of-range, NaN or self-inconsistent file is refused whole.

use glam::{Vec2, Vec3};

use super::super::super::{
    LinkEnd, NavSpan, OffMeshLink, OffMeshLinkKind, Surface, MAX_AREAS, WALKABLE_AREA,
};

const MAGIC: &[u8; 4] = b"RNAV";
pub(super) const FORMAT: u32 = 1;
/// The most a file may declare: 16k × 16k, a 4 km level at 0.25 m.
const MAX_CELLS: u64 = 1 << 28;
const PLANE: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Shape {
    pub bounds: [f32; 4],
    pub spacing: f32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Layer {
    pub cell_start: Vec<u32>,
    pub spans: Vec<NavSpan>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Saved {
    pub fingerprint: u64,
    pub shape: Shape,
    pub baked: Layer,
    pub raw: Layer,
    pub links: Vec<OffMeshLink>,
}

pub(super) fn encode(s: &Saved) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    put(&mut out, FORMAT);
    out.extend(s.fingerprint.to_le_bytes());
    s.shape.bounds.iter().for_each(|&f| putf(&mut out, f));
    putf(&mut out, s.shape.spacing);
    put(&mut out, s.shape.width);
    put(&mut out, s.shape.height);
    for layer in [&s.baked, &s.raw] {
        put(&mut out, layer.cell_start.len() as u32);
        layer.cell_start.iter().for_each(|&c| put(&mut out, c));
        put(&mut out, layer.spans.len() as u32);
        for sp in &layer.spans {
            putf(&mut out, sp.y);
            putf(&mut out, sp.ceiling);
            out.push(sp.area);
            let f = &sp.surface;
            let plane = [f.centre, f.slope.x, f.slope.y];
            if plane.iter().all(|&v| v == 0.0) {
                let signs = plane.iter().enumerate();
                out.push(signs.fold(0, |k, (i, v)| k | u8::from(v.is_sign_negative()) << i));
            } else {
                out.push(PLANE);
                plane.iter().for_each(|&v| putf(&mut out, v));
            }
            putf(&mut out, f.floor);
        }
    }
    put(&mut out, s.links.len() as u32);
    for l in &s.links {
        out.push(kind_byte(l.kind));
        (l.start.to_array().iter().chain(&l.end.to_array())).for_each(|&v| putf(&mut out, v));
        for e in [l.from, l.to] {
            out.extend(e.gx.to_le_bytes());
            out.extend(e.gz.to_le_bytes());
            put(&mut out, e.layer);
        }
        out.push(u8::from(l.bidirectional));
        putf(&mut out, l.cost_override);
        out.push(l.area);
    }
    out
}

pub(super) fn decode(bytes: &[u8]) -> Result<Saved, String> {
    let mut r = Reader(bytes);
    if r.take(4)? != MAGIC {
        return Err("not a navmesh file".into());
    }
    let format = r.u32()?;
    if format != FORMAT {
        return Err(format!("format {format}, this engine reads {FORMAT}"));
    }
    let fingerprint = u64::from_le_bytes(r.array()?);
    let bounds = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
    let shape = Shape {
        bounds,
        spacing: r.f32()?,
        width: r.u32()?,
        height: r.u32()?,
    };
    let cells = u64::from(shape.width) * u64::from(shape.height);
    let finite = bounds.iter().all(|b| b.is_finite()) && shape.spacing.is_finite();
    if !finite || shape.spacing <= 0.0 || cells == 0 || cells > MAX_CELLS {
        return Err("grid out of range".into());
    }
    let baked = layer(&mut r, cells)?;
    let raw = layer(&mut r, cells)?;
    let links = links(&mut r, &shape, &baked)?;
    if !r.0.is_empty() {
        return Err("trailing bytes".into());
    }
    Ok(Saved {
        fingerprint,
        shape,
        baked,
        raw,
        links,
    })
}

fn layer(r: &mut Reader, cells: u64) -> Result<Layer, String> {
    let starts = r.u32()? as u64;
    if starts != cells + 1 {
        return Err("cell starts disagree with the grid".into());
    }
    let cell_start = (0..starts)
        .map(|_| r.u32())
        .collect::<Result<Vec<_>, _>>()?;
    let count = r.u32()?;
    let ordered = cell_start.windows(2).all(|w| w[0] <= w[1]);
    if cell_start[0] != 0 || !ordered || cell_start.last() != Some(&count) {
        return Err("cell starts out of order".into());
    }
    let mut spans = Vec::with_capacity((count as usize).min(r.0.len() / 10));
    for _ in 0..count {
        let (y, ceiling, area) = (r.f32()?, r.f32()?, r.take(1)?[0]);
        let (centre, sx, sz) = match r.take(1)?[0] {
            PLANE => (r.f32()?, r.f32()?, r.f32()?),
            signs @ 0..PLANE => {
                let zero = |i: u8| if signs >> i & 1 == 1 { -0.0 } else { 0.0 };
                (zero(0), zero(1), zero(2))
            }
            other => return Err(format!("surface kind {other}")),
        };
        let floor = r.f32()?;
        let plane = [centre, sx, sz].iter().all(|v| v.is_finite());
        // A ceiling or a floor may be infinite (open sky, nothing under), never NaN.
        let sound = y.is_finite() && ceiling >= y && floor >= 0.0;
        if !plane || !sound || usize::from(area) >= MAX_AREAS {
            return Err("a span out of range".into());
        }
        let surface = Surface {
            centre,
            slope: Vec2::new(sx, sz),
            floor,
        };
        spans.push(NavSpan {
            y,
            ceiling,
            area,
            surface,
        });
    }
    Ok(Layer { cell_start, spans })
}

fn links(r: &mut Reader, shape: &Shape, baked: &Layer) -> Result<Vec<OffMeshLink>, String> {
    let count = r.u32()?;
    let mut out = Vec::with_capacity((count as usize).min(r.0.len() / 55));
    for _ in 0..count {
        let kind = match r.take(1)?[0] {
            1 => OffMeshLinkKind::Drop,
            2 => OffMeshLinkKind::Jump,
            other => return Err(format!("link kind {other}")),
        };
        let start = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
        let end = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
        let mut end_of = || -> Result<LinkEnd, String> {
            let gx = i32::from_le_bytes(r.array()?);
            let gz = i32::from_le_bytes(r.array()?);
            let layer = r.u32()?;
            Ok(LinkEnd { gx, gz, layer })
        };
        let (from, to) = (end_of()?, end_of()?);
        let bidirectional = match r.take(1)?[0] {
            0 => false,
            1 => true,
            other => return Err(format!("bidirectional flag {other}")),
        };
        let cost_override = r.f32()?;
        let area = r.take(1)?[0];
        let finite = start.is_finite() && end.is_finite() && cost_override.is_finite();
        let named = [from, to].iter().all(|&e| names_span(shape, baked, e));
        if !finite || !named || area != WALKABLE_AREA {
            return Err("a link out of range".into());
        }
        out.push(OffMeshLink {
            kind,
            start,
            end,
            from,
            to,
            bidirectional,
            cost_override,
            area,
            owner: None,
        });
    }
    Ok(out)
}

fn names_span(shape: &Shape, baked: &Layer, e: LinkEnd) -> bool {
    let inside =
        (0..shape.width as i32).contains(&e.gx) && (0..shape.height as i32).contains(&e.gz);
    if !inside {
        return false;
    }
    let cell = e.gz as usize * shape.width as usize + e.gx as usize;
    let len = baked.cell_start[cell + 1] - baked.cell_start[cell];
    e.layer < len
}

fn kind_byte(kind: OffMeshLinkKind) -> u8 {
    match kind {
        OffMeshLinkKind::Manual => 0,
        OffMeshLinkKind::Drop => 1,
        OffMeshLinkKind::Jump => 2,
    }
}

fn put(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_le_bytes());
}

fn putf(out: &mut Vec<u8>, v: f32) {
    out.extend(v.to_le_bytes());
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.0.len() < n {
            return Err("truncated".into());
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("took N bytes"))
    }

    fn u32(&mut self) -> Result<u32, String> {
        self.array().map(u32::from_le_bytes)
    }

    fn f32(&mut self) -> Result<f32, String> {
        self.array().map(f32::from_le_bytes)
    }
}
