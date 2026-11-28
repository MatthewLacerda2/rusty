//! Little-endian: `RNAV`, format `u32`, fingerprint `u64`, bounds and spacing `f32`, tile side
//! `u32`, then the tiles: key `i32`s, fingerprint, baked then pre-erosion columns (span count; if
//! any, cell starts and spans: floor, ceiling `f32`, area `u8`, plane sign bits or [`PLANE`] and
//! three `f32`s, floor `f32`), and the links leaving it, ends in world cells. A truncated, NaN or
//! inconsistent file is refused whole.

use glam::{Vec2, Vec3};

use super::bytes::{put, putf, Reader};

use super::super::super::grid::MAX_TILE_SIDE;
use super::super::super::{
    LinkEnd, NavSpan, OffMeshLink, OffMeshLinkKind, Surface, TileKey, MAX_AREAS, WALKABLE_AREA,
};

const MAGIC: &[u8; 4] = b"RNAV";
pub(super) const FORMAT: u32 = 3;
const MAX_TILE_CELLS: u64 = (MAX_TILE_SIDE as u64) * (MAX_TILE_SIDE as u64);
const PLANE: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Shape {
    pub bounds: [f32; 4],
    pub spacing: f32,
    pub side: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Layer {
    pub cell_start: Vec<u32>,
    pub spans: Vec<NavSpan>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SavedTile {
    pub key: TileKey,
    pub fingerprint: u64,
    pub baked: Layer,
    pub raw: Layer,
    pub links: Vec<OffMeshLink>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Saved {
    pub fingerprint: u64,
    pub shape: Shape,
    pub tiles: Vec<SavedTile>,
}

pub(super) fn encode(s: &Saved) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    put(&mut out, FORMAT);
    out.extend(s.fingerprint.to_le_bytes());
    s.shape.bounds.iter().for_each(|&f| putf(&mut out, f));
    putf(&mut out, s.shape.spacing);
    put(&mut out, s.shape.side);
    put(&mut out, s.tiles.len() as u32);
    for t in &s.tiles {
        out.extend(t.key.0.to_le_bytes());
        out.extend(t.key.1.to_le_bytes());
        out.extend(t.fingerprint.to_le_bytes());
        for layer in [&t.baked, &t.raw] {
            put(&mut out, layer.spans.len() as u32);
            if layer.spans.is_empty() {
                continue;
            }
            layer.cell_start.iter().for_each(|&c| put(&mut out, c));
            layer.spans.iter().for_each(|sp| span(&mut out, sp));
        }
        put(&mut out, t.links.len() as u32);
        t.links.iter().for_each(|l| link(&mut out, l));
    }
    out
}

fn span(out: &mut Vec<u8>, sp: &NavSpan) {
    putf(out, sp.y);
    putf(out, sp.ceiling);
    out.push(sp.area);
    let f = &sp.surface;
    let plane = [f.centre, f.slope.x, f.slope.y];
    if plane.iter().all(|&v| v == 0.0) {
        let signs = plane.iter().enumerate();
        out.push(signs.fold(0, |k, (i, v)| k | u8::from(v.is_sign_negative()) << i));
    } else {
        out.push(PLANE);
        plane.iter().for_each(|&v| putf(out, v));
    }
    putf(out, f.floor);
}

fn link(out: &mut Vec<u8>, l: &OffMeshLink) {
    out.push(kind_byte(l.kind));
    (l.start.to_array().iter().chain(&l.end.to_array())).for_each(|&v| putf(out, v));
    for e in [l.from, l.to] {
        out.extend(e.gx.to_le_bytes());
        out.extend(e.gz.to_le_bytes());
        put(out, e.layer);
    }
    out.push(u8::from(l.bidirectional));
    putf(out, l.cost_override);
    out.push(l.area);
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
        side: r.u32()?,
    };
    let cells = u64::from(shape.side) * u64::from(shape.side);
    let finite = bounds.iter().all(|b| b.is_finite()) && shape.spacing.is_finite();
    if !finite || shape.spacing <= 0.0 || cells == 0 || cells > MAX_TILE_CELLS {
        return Err("grid out of range".into());
    }
    let count = r.u32()?;
    let mut tiles: Vec<SavedTile> = Vec::with_capacity((count as usize).min(r.0.len() / 28));
    for _ in 0..count {
        let key = (
            i32::from_le_bytes(r.array()?),
            i32::from_le_bytes(r.array()?),
        );
        if tiles.last().is_some_and(|t| t.key >= key) {
            return Err("tiles out of order".into());
        }
        let fingerprint = u64::from_le_bytes(r.array()?);
        let baked = layer(&mut r, cells)?;
        let raw = layer(&mut r, cells)?;
        let links = links(&mut r, key, shape.side as i32)?;
        tiles.push(SavedTile {
            key,
            fingerprint,
            baked,
            raw,
            links,
        });
    }
    if !r.0.is_empty() {
        return Err("trailing bytes".into());
    }
    Ok(Saved {
        fingerprint,
        shape,
        tiles,
    })
}

pub(super) fn first_baked_span(bytes: &[u8]) -> Option<usize> {
    let mut r = Reader(bytes);
    r.take(36).ok()?;
    let side = r.u32().ok()?;
    let cells = u64::from(side) * u64::from(side);
    for _ in 0..r.u32().ok()? {
        let key = (
            i32::from_le_bytes(r.array().ok()?),
            i32::from_le_bytes(r.array().ok()?),
        );
        r.take(8).ok()?;
        let count = u32::from_le_bytes(r.0.get(..4)?.try_into().ok()?);
        if count > 0 {
            return Some(bytes.len() - r.0.len() + 4 + (cells as usize + 1) * 4);
        }
        layer(&mut r, cells).ok()?;
        layer(&mut r, cells).ok()?;
        links(&mut r, key, side as i32).ok()?;
    }
    None
}

fn layer(r: &mut Reader, cells: u64) -> Result<Layer, String> {
    let count = r.u32()?;
    if count == 0 {
        return Ok(Layer::default());
    }
    let starts = r.take(usize::try_from((cells + 1) * 4).map_err(|_| "truncated")?)?;
    let word = |c: &[u8]| u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
    let cell_start: Vec<u32> = starts.chunks_exact(4).map(word).collect();
    let ordered = cell_start.windows(2).all(|w| w[0] <= w[1]);
    if cell_start[0] != 0 || !ordered || cell_start.last() != Some(&count) {
        return Err("cell starts out of order".into());
    }
    let mut spans = Vec::with_capacity((count as usize).min(r.0.len() / 14));
    for _ in 0..count {
        spans.push(span_of(r)?);
    }
    Ok(Layer { cell_start, spans })
}

/// One span, read off a slice: the bulk of a file, so no `Reader` call per field.
fn span_of(r: &mut Reader) -> Result<NavSpan, String> {
    let b = r.0;
    let at = |i: usize| f32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let kind = *b.get(9).ok_or("truncated")?;
    let len = if kind == PLANE { 26 } else { 14 };
    if b.len() < len {
        return Err("truncated".into());
    }
    let (y, ceiling, area) = (at(0), at(4), b[8]);
    let (centre, sx, sz) = match kind {
        PLANE => (at(10), at(14), at(18)),
        signs @ 0..PLANE => {
            let zero = |i: u8| if signs >> i & 1 == 1 { -0.0 } else { 0.0 };
            (zero(0), zero(1), zero(2))
        }
        other => return Err(format!("surface kind {other}")),
    };
    let floor = at(len - 4);
    r.0 = &b[len..];
    let plane = [centre, sx, sz].iter().all(|v| v.is_finite());
    // A ceiling or a floor may be infinite (open sky, nothing under), never NaN.
    let sound = y.is_finite() && ceiling >= y && floor >= 0.0;
    if !plane || !sound || usize::from(area) >= MAX_AREAS {
        return Err("a span out of range".into());
    }
    let slope = Vec2::new(sx, sz);
    let surface = Surface {
        centre,
        slope,
        floor,
    };
    Ok(NavSpan {
        y,
        ceiling,
        area,
        surface,
    })
}

fn links(r: &mut Reader, key: TileKey, side: i32) -> Result<Vec<OffMeshLink>, String> {
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
        let home = (from.gx.div_euclid(side), from.gz.div_euclid(side)) == key;
        if !finite || !home || area != WALKABLE_AREA {
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

fn kind_byte(kind: OffMeshLinkKind) -> u8 {
    match kind {
        OffMeshLinkKind::Manual => 0,
        OffMeshLinkKind::Drop => 1,
        OffMeshLinkKind::Jump => 2,
    }
}
