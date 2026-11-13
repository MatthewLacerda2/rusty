//! [`quad_mode_gltf`](super::quad_mode_gltf)'s one quad, written to a folder of its own.

/// A one-quad glTF (no `TEXCOORD_1`) in its own folder `dir` of the test temp dir,
/// emptied first, with no sidecar yet. Returns the `.gltf`'s path.
pub fn quad_file(dir: &str) -> String {
    let dir = crate::temp::dir().join(dir);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the quad's folder");
    let (json, bin) = super::quad_mode_gltf(4, &[0, 1, 2, 0, 2, 3]);
    std::fs::write(dir.join("quad.bin"), bin).expect("write quad.bin");
    let path = dir.join("quad.gltf");
    std::fs::write(&path, json).expect("write quad.gltf");
    path.to_string_lossy().into_owned()
}
