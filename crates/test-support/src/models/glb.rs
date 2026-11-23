//! A glTF document and its one buffer packed as a binary `.glb`, so a fixture imports
//! from memory.

/// `json` is a `.gltf` document whose buffer 0 has no `uri`: a `.glb`'s `BIN` chunk,
/// `bin`, is that buffer.
pub fn glb(json: &str, bin: &[u8]) -> Vec<u8> {
    let chunk = |mut bytes: Vec<u8>, pad: u8, kind: &[u8; 4]| {
        bytes.resize(bytes.len().next_multiple_of(4), pad);
        let mut out = (bytes.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend(bytes);
        out
    };
    let mut body = chunk(json.as_bytes().to_vec(), b' ', b"JSON");
    if !bin.is_empty() {
        body.extend(chunk(bin.to_vec(), 0, b"BIN\0"));
    }
    let mut out = b"glTF".to_vec();
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(12 + body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}
