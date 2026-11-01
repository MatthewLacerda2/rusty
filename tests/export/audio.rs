//! No `.mp3` ships (#1653): the export converts each to a `.wav` first, so one it
//! can't convert fails the export, naming it, and one it leaves because its `.wav`
//! already exists is warned of, naming both, while the `.wav` ships.

use super::{export, project, tree};

/// Put `bytes` at `rel` under `game`.
fn put(game: &std::path::Path, rel: &str, bytes: &[u8]) {
    let path = game.join(rusty::core::paths::native(rel));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn an_mp3_that_does_not_decode_fails_the_export_naming_it() {
    let game = project(&[], "");
    put(&game, "assets/audio/half_copied.mp3", b"not audio yet");
    let out = crate::temp::dir().join("out");
    let exported = export(&game, &out);
    assert!(!exported.status.success(), "{exported:?}");
    let stderr = String::from_utf8_lossy(&exported.stderr);
    assert!(stderr.contains("half_copied.mp3"), "{stderr}");
    let written = std::fs::read_dir(&out).map_or(0, |entries| entries.count());
    assert_eq!(written, 0, "nothing is written");
}

#[test]
fn an_mp3_beside_its_wav_is_warned_of_and_the_wav_ships() {
    let game = project(&[], "");
    put(&game, "assets/audio/theme.mp3", b"the editor's take");
    put(&game, "assets/audio/theme.wav", b"somebody's file");
    let out = crate::temp::dir().join("out");
    let exported = export(&game, &out);
    assert!(exported.status.success(), "{exported:?}");
    let stderr = String::from_utf8_lossy(&exported.stderr);
    let named = ["theme.mp3", "theme.wav"]
        .iter()
        .all(|f| stderr.contains(f));
    assert!(named, "the warning names both: {stderr}");
    let shipped = tree(&out);
    let wav = shipped.get("assets/audio/theme.wav").map(Vec::as_slice);
    assert_eq!(wav, Some(&b"somebody's file"[..]), "kept as it was");
    assert!(!shipped.contains_key("assets/audio/theme.mp3"));
}
