//! The BIN chunk under construction, with the buffer views and accessors into it.

use serde_json::{json, Value};

const FLOAT: u32 = 5126;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
/// No buffer-view target: data that is not a vertex or index stream (inverse
/// bind matrices, animation samplers) must not name one.
const NO_TARGET: u32 = 0;

#[derive(Default)]
pub(super) struct Bin {
    pub bytes: Vec<u8>,
    pub views: Vec<Value>,
    pub accessors: Vec<Value>,
}

impl Bin {
    /// Append `data` as a new buffer view and return its accessor index.
    fn push(&mut self, data: Vec<u8>, target: u32, accessor: Value) -> usize {
        pad(&mut self.bytes, 0);
        let mut view =
            json!({"buffer": 0, "byteOffset": self.bytes.len(), "byteLength": data.len()});
        if target != NO_TARGET {
            view["target"] = json!(target);
        }
        self.bytes.extend_from_slice(&data);
        let mut accessor = accessor;
        accessor["bufferView"] = json!(self.views.len());
        self.views.push(view);
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    /// A vertex stream of `N`-component floats; `bounds` adds the `min`/`max`
    /// glTF requires on `POSITION`.
    pub fn floats<const N: usize>(
        &mut self,
        items: &[[f32; N]],
        kind: &str,
        bounds: bool,
    ) -> usize {
        self.float_accessor(items, kind, bounds, ARRAY_BUFFER)
    }

    /// Floats that are not a vertex stream: inverse bind matrices, animation
    /// sampler outputs.
    pub fn data<const N: usize>(&mut self, items: &[[f32; N]], kind: &str) -> usize {
        self.float_accessor(items, kind, false, NO_TARGET)
    }

    /// An animation sampler's input: key times, with the bounds glTF requires.
    pub fn times(&mut self, times: &[f32]) -> usize {
        let items: Vec<[f32; 1]> = times.iter().map(|&t| [t]).collect();
        self.float_accessor(&items, "SCALAR", true, NO_TARGET)
    }

    fn float_accessor<const N: usize>(
        &mut self,
        items: &[[f32; N]],
        kind: &str,
        bounds: bool,
        target: u32,
    ) -> usize {
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
        self.push(data, target, accessor)
    }

    pub fn joints(&mut self, joints: &[[u16; 4]]) -> usize {
        let data = joints
            .iter()
            .flatten()
            .flat_map(|j| j.to_le_bytes())
            .collect();
        let accessor =
            json!({"componentType": UNSIGNED_SHORT, "count": joints.len(), "type": "VEC4"});
        self.push(data, ARRAY_BUFFER, accessor)
    }

    pub fn indices(&mut self, indices: &[u32]) -> usize {
        let data = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let accessor =
            json!({"componentType": UNSIGNED_INT, "count": indices.len(), "type": "SCALAR"});
        self.push(data, ELEMENT_ARRAY_BUFFER, accessor)
    }
}

/// Pad `bytes` with `with` to a 4-byte boundary.
pub(super) fn pad(bytes: &mut Vec<u8>, with: u8) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(with);
    }
}
