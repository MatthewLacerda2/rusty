//! A cooked model as a binary glTF (`.glb`): one mesh per sub-mesh, one primitive each.
//!
//! `asset.extras` holds the cooker version and stamp; `extras.parse` the digest of the
//! parse; `extras.source` (only when the unwrap ran) the parsed vertex each vertex
//! copies; `extras.clusters` the range and bounds accessors. Floats are stored as
//! little-endian bits. A file with another stamp or a misfit accessor is refused whole
//! and the model cooked again. Empty sub-meshes write count-0 accessors, the one place
//! the file leaves strict glTF.

use std::ops::Range;
use std::sync::Arc;

use serde_json::{json, Value};

use super::stamp::COOKER_VERSION;
use super::sub::Cooked;
use crate::clusters::{Cluster, Clusters};
use crate::convert::glb::{container, Bin};

const MAGIC: u32 = 0x4654_6C67; // "glTF"
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;
const FLOAT: u64 = 5126;
const UNSIGNED_INT: u64 = 5125;
const BOUNDS: usize = 13;

/// Why a file was refused as made from something else: expected after an edit, not malformed.
pub const STALE: &str = "stale: ";

pub fn encode(stamp: u64, subs: &[Cooked]) -> Vec<u8> {
    let mut bin = Bin::default();
    let meshes: Vec<Value> = subs.iter().map(|s| mesh(s, &mut bin)).collect();
    let mut root = json!({
        "asset": {
            "version": "2.0",
            "generator": "rusty cook",
            "extras": {"cooker": COOKER_VERSION, "stamp": format!("{stamp:016x}")},
        },
        "meshes": meshes,
    });
    if !bin.views.is_empty() {
        root["buffers"] = json!([{"byteLength": bin.bytes.len()}]);
        root["bufferViews"] = Value::Array(bin.views);
        root["accessors"] = Value::Array(bin.accessors);
    }
    let json = serde_json::to_vec(&root).unwrap_or_default();
    container(json, bin.bytes)
}

fn mesh(sub: &Cooked, bin: &mut Bin) -> Value {
    let attributes = json!({
        "POSITION": bin.floats(&sub.positions, "VEC3", true),
        "TANGENT": bin.floats(&sub.tangents, "VEC4", false),
        "TEXCOORD_1": bin.floats(&sub.lightmap_uvs, "VEC2", false),
    });
    let mut primitive = json!({"attributes": attributes, "indices": bin.indices(&sub.indices)});
    primitive["extras"] = json!({"parse": format!("{:016x}", sub.parse)});
    if let Some(source) = &sub.source {
        primitive["extras"]["source"] = json!(bin.uints(source));
    }
    if let Some(clusters) = &sub.clusters {
        let ranges: Vec<u32> = (clusters.list.iter())
            .flat_map(|c| [c.indices.start, c.indices.end])
            .collect();
        let bounds: Vec<[f32; 1]> = (clusters.list.iter())
            .flat_map(|c| bounds_of(c).map(|f| [f]))
            .collect();
        primitive["extras"]["clusters"] = json!({
            "ranges": bin.uints(&ranges),
            "bounds": bin.data(&bounds, "SCALAR"),
        });
    }
    json!({"name": sub.id, "primitives": [primitive]})
}

fn bounds_of(c: &Cluster) -> [f32; BOUNDS] {
    let mut out = [0.0; BOUNDS];
    let parts = [&c.min[..], &c.max, &c.apex, &c.axis, &[c.cutoff]];
    for (o, v) in out.iter_mut().zip(parts.into_iter().flatten()) {
        *o = *v;
    }
    out
}

pub fn parse(bytes: &[u8], stamp: u64) -> Result<Vec<Cooked>, String> {
    let (json, bin) = chunks(bytes)?;
    let root: Value = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    stale(&root, stamp)?;
    let file = File { root: &root, bin };
    let meshes = root["meshes"].as_array().ok_or("no meshes")?;
    meshes.iter().map(|m| file.sub(m)).collect()
}

fn stale(root: &Value, stamp: u64) -> Result<(), String> {
    let extras = &root["asset"]["extras"];
    if extras["cooker"] != json!(COOKER_VERSION) {
        return Err(format!("{STALE}cooked by version {}", extras["cooker"]));
    }
    if extras["stamp"] != json!(format!("{stamp:016x}")) {
        return Err(format!("{STALE}cooked from another source or settings"));
    }
    Ok(())
}

pub fn stamped(path: &std::path::Path, stamp: u64) -> bool {
    let json = crate::import_cache::glb_json(path);
    let root = json.and_then(|j| serde_json::from_slice::<Value>(&j).ok());
    root.is_some_and(|r| stale(&r, stamp).is_ok())
}

fn chunks(bytes: &[u8]) -> Result<(&[u8], &[u8]), String> {
    let word = |at: usize| -> Result<u32, String> {
        let b = bytes.get(at..at + 4).ok_or("cut short")?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if word(0)? != MAGIC || word(8)? as usize != bytes.len() {
        return Err("not a whole .glb".to_string());
    }
    let mut at = 12;
    let mut chunk = |kind: u32| -> Result<Option<&[u8]>, String> {
        if at == bytes.len() {
            return Ok(None);
        }
        let len = word(at)? as usize;
        if word(at + 4)? != kind {
            return Err("an unexpected chunk".to_string());
        }
        let body = bytes
            .get(at + 8..at + 8 + len)
            .ok_or("a chunk past the end")?;
        at += 8 + len;
        Ok(Some(body))
    };
    let json = chunk(CHUNK_JSON)?.ok_or("no JSON chunk")?;
    let bin = chunk(CHUNK_BIN)?.unwrap_or(&[]);
    Ok((json, bin))
}

struct File<'a> {
    root: &'a Value,
    bin: &'a [u8],
}

impl File<'_> {
    fn sub(&self, mesh: &Value) -> Result<Cooked, String> {
        let id = mesh["name"].as_str().ok_or("a mesh with no name")?;
        let p = &mesh["primitives"][0];
        let a = &p["attributes"];
        let parse = p["extras"]["parse"]
            .as_str()
            .and_then(|h| u64::from_str_radix(h, 16).ok());
        let source = match p["extras"].get("source") {
            Some(i) => Some(self.uints(i)?),
            None => None,
        };
        let clusters = match p["extras"].get("clusters") {
            Some(c) => Some(Arc::new(self.clusters(c)?)),
            None => None,
        };
        Ok(Cooked {
            id: id.to_string(),
            parse: parse.ok_or("a mesh with no parse digest")?,
            source,
            positions: self.floats(&a["POSITION"], "VEC3")?,
            tangents: self.floats(&a["TANGENT"], "VEC4")?,
            lightmap_uvs: self.floats(&a["TEXCOORD_1"], "VEC2")?,
            indices: self.uints(&p["indices"])?,
            clusters,
        })
    }

    fn clusters(&self, c: &Value) -> Result<Clusters, String> {
        let ranges = self.uints(&c["ranges"])?;
        let bounds: Vec<[f32; 1]> = self.floats(&c["bounds"], "SCALAR")?;
        if ranges.len() % 2 != 0 || ranges.len() / 2 * BOUNDS != bounds.len() {
            return Err("a cluster table whose two halves disagree".to_string());
        }
        let list = ranges.chunks_exact(2).zip(bounds.chunks_exact(BOUNDS));
        let cluster = |(r, b): (&[u32], &[[f32; 1]])| {
            let f = |at: usize| [b[at][0], b[at + 1][0], b[at + 2][0]];
            Cluster {
                indices: r[0]..r[1],
                min: f(0),
                max: f(3),
                apex: f(6),
                axis: f(9),
                cutoff: b[12][0],
            }
        };
        Ok(Clusters {
            list: list.map(cluster).collect(),
        })
    }

    fn bytes(
        &self,
        index: &Value,
        component: u64,
        kind: &str,
        width: usize,
    ) -> Result<&[u8], String> {
        let at = index.as_u64().ok_or("a missing accessor")? as usize;
        let acc = &self.root["accessors"][at];
        if acc["componentType"] != json!(component) || acc["type"] != json!(kind) {
            return Err(format!("accessor {at} is not {kind} of {component}"));
        }
        let count = acc["count"].as_u64().ok_or("an accessor with no count")? as usize;
        let view =
            &self.root["bufferViews"][acc["bufferView"].as_u64().unwrap_or(u64::MAX) as usize];
        let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let len = view["byteLength"].as_u64().ok_or("a view with no length")? as usize;
        if count.checked_mul(width) != Some(len) {
            return Err(format!("accessor {at}'s view is not {count} elements"));
        }
        let range: Range<usize> = offset..offset.checked_add(len).ok_or("an overflowing view")?;
        self.bin
            .get(range)
            .ok_or_else(|| format!("accessor {at} past the buffer"))
    }

    fn floats<const N: usize>(&self, index: &Value, kind: &str) -> Result<Vec<[f32; N]>, String> {
        let bytes = self.bytes(index, FLOAT, kind, 4 * N)?;
        let float = |b: &[u8]| f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let item = |c: &[u8]| std::array::from_fn(|i| float(&c[i * 4..]));
        Ok(bytes.chunks_exact(4 * N).map(item).collect())
    }

    fn uints(&self, index: &Value) -> Result<Vec<u32>, String> {
        let bytes = self.bytes(index, UNSIGNED_INT, "SCALAR", 4)?;
        Ok(bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect())
    }
}
