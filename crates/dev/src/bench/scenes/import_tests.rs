use std::os::unix::fs::symlink;

use super::*;

fn root(name: &str) -> PathBuf {
    let root = rusty_test_support::temp::dir().join(format!("{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn the_copy_leaves_the_caches_out() {
    let root = root("import");
    let (project, copy) = (root.join("game"), root.join("copy"));
    for dir in ["assets/scenes", IMPORT_CACHE_DIR, CACHE_DIR, ".git"] {
        std::fs::create_dir_all(project.join(dir)).unwrap();
    }
    std::fs::write(project.join("assets/scenes/a.scene"), "{}").unwrap();
    std::fs::write(project.join(IMPORT_CACHE_DIR).join("m.glb"), "glb").unwrap();
    std::fs::write(project.join("cache/lock"), "").unwrap();
    scratch(&project, &copy).unwrap();
    assert!(copy.join("assets/scenes/a.scene").is_file());
    for left in LEFT_OUT {
        assert!(!copy.join(left).exists(), "{left} copied");
    }
    assert!(
        project.join(IMPORT_CACHE_DIR).join("m.glb").is_file(),
        "untouched"
    );
}

#[test]
fn links_are_copied_as_what_they_point_to_and_a_loop_or_a_socket_is_left_out() {
    let root = root("import_links");
    let (project, copy, shared) = (root.join("game"), root.join("copy"), root.join("shared"));
    std::fs::create_dir_all(project.join("assets")).unwrap();
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(shared.join("tex.png"), "png").unwrap();
    symlink("../../shared", project.join("assets/textures")).unwrap();
    symlink(shared.join("tex.png"), project.join("assets/logo.png")).unwrap();
    symlink(&project, project.join("assets/up")).unwrap();
    symlink(root.join("gone"), project.join("assets/dangling")).unwrap();
    // lint: allow(test-temp-dir) a socket path must fit SUN_LEN, which the per-test dir overruns; named by this process's pid, which nextest gives one test
    let sock = std::env::temp_dir().join(format!("rb{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock);
    let _socket = std::os::unix::net::UnixListener::bind(&sock).unwrap();
    symlink(&sock, project.join("assets/sock")).unwrap();
    scratch(&project, &copy).unwrap();
    let textures = copy.join("assets/textures");
    assert!(
        !textures.is_symlink(),
        "a folder link is copied as a folder"
    );
    assert_eq!(
        std::fs::read_to_string(textures.join("tex.png")).unwrap(),
        "png"
    );
    let logo = copy.join("assets/logo.png");
    assert!(!logo.is_symlink(), "a file link is copied as a file");
    std::fs::write(&logo, "rewritten").unwrap();
    assert_eq!(
        std::fs::read_to_string(shared.join("tex.png")).unwrap(),
        "png",
        "a write in the copy never reaches the original"
    );
    for left in ["up", "dangling", "sock"] {
        assert!(!copy.join("assets").join(left).exists(), "{left} copied");
    }
    let _ = std::fs::remove_file(&sock);
}
