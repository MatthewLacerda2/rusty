//! What looking at a dependency's path finds (#1189): only a regular file is
//! read, and the stamp must be able to name it.

use super::super::{Dependency, Seen};

#[test]
fn a_folder_is_absent_not_read() {
    let dir = crate::test_temp::dir();
    let looked = Dependency::look(&dir).unwrap();
    assert_eq!(looked.seen(), Seen::Absent);
}

#[cfg(unix)]
#[test]
fn a_device_that_never_ends_is_absent_not_read() {
    // Read whole, `/dev/zero` would never end.
    let looked = Dependency::look(std::path::Path::new("/dev/zero")).unwrap();
    assert_eq!(looked.seen(), Seen::Absent);
}

#[cfg(unix)]
#[test]
fn a_path_that_is_not_utf8_is_refused() {
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::OsStr::from_bytes(b"skin\xff.png");
    assert!(Dependency::look(&crate::test_temp::dir().join(name)).is_err());
}

#[test]
fn a_file_is_read_with_its_digest() {
    let file = crate::test_temp::dir().join("skin.png");
    std::fs::write(&file, b"red").unwrap();
    let digest = crate::core::hash::fnv1a64(b"red");
    assert_eq!(Dependency::look(&file).unwrap().seen(), Seen::Read(digest));
}

#[test]
fn a_file_the_engine_rewrites_is_read_again_at_the_same_length() {
    let file = crate::test_temp::dir().join("skin.png");
    std::fs::write(&file, b"red").unwrap();
    assert_eq!(
        Dependency::look(&file).unwrap().seen(),
        Seen::Read(crate::core::hash::fnv1a64(b"red"))
    );
    // Same length: only the write log tells the memo the file changed (#1219).
    crate::core::fs::written::write(&file, b"tan").unwrap();
    let digest = crate::core::hash::fnv1a64(b"tan");
    assert_eq!(Dependency::look(&file).unwrap().seen(), Seen::Read(digest));
}
