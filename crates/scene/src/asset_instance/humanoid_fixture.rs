//! A humanoid model borrowing clips (#1747), for the tests here and the scripting
//! tests' `Assets.SetHumanoid`. A test hook: compiled in every build, since those
//! tests are another crate's; the glTF it copies is the caller's (the test-support
//! crate's animated fixture), which only tests link.

use std::path::{Path, PathBuf};

/// `name`, a copy of the two-joint animated glTF `gltf` (its buffer `bin` beside it
/// as `anim.bin`) whose one clip is `clip`, in `dir`, its sidecar mapping `Joint0`
/// to the hips with `clips_from`.
pub fn write(
    dir: &Path,
    name: &str,
    clip: &str,
    clips_from: &[&Path],
    (gltf, bin): (&str, &[u8]),
) -> Result<PathBuf, String> {
    let err = |e: std::io::Error| e.to_string();
    std::fs::create_dir_all(dir).map_err(err)?;
    std::fs::write(dir.join("anim.bin"), bin).map_err(err)?;
    let path = dir.join(name);
    let text = gltf.replace("\"name\": \"Wave\"", &format!("\"name\": \"{clip}\""));
    std::fs::write(&path, text).map_err(err)?;
    let mut humanoid = rusty_asset::humanoid::HumanoidSettings::default();
    humanoid.bones.insert("Hips".into(), "Joint0".into());
    humanoid.bones.insert("Spine".into(), "Joint1".into());
    humanoid.clips_from = clips_from
        .iter()
        .map(|p| p.to_string_lossy().into())
        .collect();
    let settings = rusty_asset::sidecar::ImportSettings {
        humanoid,
        ..Default::default()
    };
    rusty_asset::sidecar::save(&path, &settings).map_err(|e| e.to_string())?;
    Ok(path)
}
