//! Serialise a [`Document`] as a binary glTF (`.glb`): one JSON chunk, one BIN
//! chunk, every accessor tightly packed and 4-byte aligned (glTF 2.0 §4.4).

use super::{Document, Material, Mesh, Node, Primitive, Skin};
use serde_json::{json, Map, Value};

const GLB_MAGIC: u32 = 0x4654_6C67; // "glTF"
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;
const FLOAT: u32 = 5126;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;

/// `doc` as the bytes of a `.glb` file.
pub fn write(doc: &Document) -> Vec<u8> {
    let mut bin = Bin::default();
    let meshes: Vec<Value> = doc.meshes.iter().map(|m| mesh(m, &mut bin)).collect();
    let skins: Vec<Value> = doc.skins.iter().map(|s| skin(s, &mut bin)).collect();
    let mut images = Vec::new();
    let materials: Vec<Value> = doc
        .materials
        .iter()
        .map(|m| material(m, &mut images))
        .collect();

    let mut root = Map::new();
    root.insert(
        "asset".into(),
        json!({"version": "2.0", "generator": "rusty import"}),
    );
    root.insert("scene".into(), json!(0));
    root.insert("scenes".into(), json!([{"nodes": doc.roots}]));
    root.insert("nodes".into(), doc.nodes.iter().map(node).collect());
    insert_nonempty(&mut root, "meshes", meshes);
    insert_nonempty(&mut root, "skins", skins);
    insert_nonempty(&mut root, "materials", materials);
    let textures: Vec<Value> = (0..images.len()).map(|i| json!({"source": i})).collect();
    insert_nonempty(&mut root, "textures", textures);
    let images: Vec<Value> = images.into_iter().map(|uri| json!({"uri": uri})).collect();
    insert_nonempty(&mut root, "images", images);
    if !bin.bytes.is_empty() {
        root.insert("buffers".into(), json!([{"byteLength": bin.bytes.len()}]));
        root.insert("bufferViews".into(), Value::Array(bin.views));
        root.insert("accessors".into(), Value::Array(bin.accessors));
    }
    let json = serde_json::to_vec(&Value::Object(root)).unwrap_or_default();
    container(json, bin.bytes)
}

fn insert_nonempty(root: &mut Map<String, Value>, key: &str, values: Vec<Value>) {
    if !values.is_empty() {
        root.insert(key.into(), Value::Array(values));
    }
}

/// The GLB container around the two chunks: JSON padded with spaces, BIN with
/// zeros, each to a 4-byte boundary.
fn container(mut json: Vec<u8>, mut bin: Vec<u8>) -> Vec<u8> {
    pad(&mut json, b' ');
    pad(&mut bin, 0);
    let mut chunks = vec![(CHUNK_JSON, json)];
    if !bin.is_empty() {
        chunks.push((CHUNK_BIN, bin));
    }
    let length = 12 + chunks.iter().map(|(_, c)| 8 + c.len()).sum::<usize>();
    let mut out = Vec::with_capacity(length);
    for word in [GLB_MAGIC, 2, length as u32] {
        out.extend_from_slice(&word.to_le_bytes());
    }
    for (kind, chunk) in chunks {
        out.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&chunk);
    }
    out
}

fn pad(bytes: &mut Vec<u8>, with: u8) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(with);
    }
}

fn node(node: &Node) -> Value {
    let mut out = Map::new();
    out.insert("name".into(), json!(node.name));
    out.insert("translation".into(), json!(node.translation));
    out.insert("rotation".into(), json!(node.rotation));
    out.insert("scale".into(), json!(node.scale));
    if !node.children.is_empty() {
        out.insert("children".into(), json!(node.children));
    }
    if let Some(mesh) = node.mesh {
        out.insert("mesh".into(), json!(mesh));
    }
    if let Some(skin) = node.skin {
        out.insert("skin".into(), json!(skin));
    }
    Value::Object(out)
}

fn mesh(mesh: &Mesh, bin: &mut Bin) -> Value {
    let primitives: Vec<Value> = mesh.primitives.iter().map(|p| primitive(p, bin)).collect();
    json!({"name": mesh.name, "primitives": primitives})
}

fn primitive(p: &Primitive, bin: &mut Bin) -> Value {
    let mut attributes = Map::new();
    let position = bin.floats(&p.positions, "VEC3", true);
    attributes.insert("POSITION".into(), json!(position));
    if !p.normals.is_empty() {
        attributes.insert(
            "NORMAL".into(),
            json!(bin.floats(&p.normals, "VEC3", false)),
        );
    }
    if !p.uvs.is_empty() {
        attributes.insert(
            "TEXCOORD_0".into(),
            json!(bin.floats(&p.uvs, "VEC2", false)),
        );
    }
    if !p.joints.is_empty() {
        attributes.insert("JOINTS_0".into(), json!(bin.joints(&p.joints)));
        attributes.insert(
            "WEIGHTS_0".into(),
            json!(bin.floats(&p.weights, "VEC4", false)),
        );
    }
    let mut out = Map::new();
    out.insert("attributes".into(), Value::Object(attributes));
    out.insert("indices".into(), json!(bin.indices(&p.indices)));
    if let Some(material) = p.material {
        out.insert("material".into(), json!(material));
    }
    Value::Object(out)
}

fn skin(skin: &Skin, bin: &mut Bin) -> Value {
    let matrices = bin.floats(&skin.inverse_bind, "MAT4", false);
    json!({"joints": skin.joints, "inverseBindMatrices": matrices})
}

/// A material, its texture URIs added to `images` (one image per distinct URI).
fn material(m: &Material, images: &mut Vec<String>) -> Value {
    let mut texture = |uri: &String| {
        let index = images.iter().position(|u| u == uri).unwrap_or_else(|| {
            images.push(uri.clone());
            images.len() - 1
        });
        json!({"index": index})
    };
    let mut pbr = Map::new();
    pbr.insert("baseColorFactor".into(), json!(m.base_color));
    pbr.insert("metallicFactor".into(), json!(m.metallic));
    pbr.insert("roughnessFactor".into(), json!(m.roughness));
    if let Some(uri) = &m.base_color_texture {
        pbr.insert("baseColorTexture".into(), texture(uri));
    }
    let mut out = Map::new();
    out.insert("name".into(), json!(m.name));
    out.insert("pbrMetallicRoughness".into(), Value::Object(pbr));
    out.insert("emissiveFactor".into(), json!(m.emissive));
    if let Some(uri) = &m.normal_texture {
        out.insert("normalTexture".into(), texture(uri));
    }
    Value::Object(out)
}

/// The BIN chunk under construction, with the views and accessors into it.
#[derive(Default)]
struct Bin {
    bytes: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Bin {
    /// Append `data` as a new buffer view and return its accessor index.
    fn push(&mut self, data: Vec<u8>, target: u32, accessor: Value) -> usize {
        pad(&mut self.bytes, 0);
        let mut view =
            json!({"buffer": 0, "byteOffset": self.bytes.len(), "byteLength": data.len()});
        if target != 0 {
            view["target"] = json!(target);
        }
        self.bytes.extend_from_slice(&data);
        let mut accessor = accessor;
        accessor["bufferView"] = json!(self.views.len());
        self.views.push(view);
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    /// A float accessor of `N`-component elements; `bounds` adds the `min`/`max`
    /// glTF requires on `POSITION`.
    fn floats<const N: usize>(&mut self, items: &[[f32; N]], kind: &str, bounds: bool) -> usize {
        let data = items
            .iter()
            .flatten()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let mut accessor = json!({"componentType": FLOAT, "count": items.len(), "type": kind});
        if bounds && !items.is_empty() {
            let fold = |pick: fn(f32, f32) -> f32| {
                (0..N)
                    .map(|i| items.iter().map(|v| v[i]).reduce(pick).unwrap_or(0.0))
                    .collect::<Vec<f32>>()
            };
            accessor["min"] = json!(fold(f32::min));
            accessor["max"] = json!(fold(f32::max));
        }
        let target = if kind == "MAT4" { 0 } else { ARRAY_BUFFER };
        self.push(data, target, accessor)
    }

    fn joints(&mut self, joints: &[[u16; 4]]) -> usize {
        let data = joints
            .iter()
            .flatten()
            .flat_map(|j| j.to_le_bytes())
            .collect();
        let accessor =
            json!({"componentType": UNSIGNED_SHORT, "count": joints.len(), "type": "VEC4"});
        self.push(data, ARRAY_BUFFER, accessor)
    }

    fn indices(&mut self, indices: &[u32]) -> usize {
        let data = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let accessor =
            json!({"componentType": UNSIGNED_INT, "count": indices.len(), "type": "SCALAR"});
        self.push(data, ELEMENT_ARRAY_BUFFER, accessor)
    }
}
