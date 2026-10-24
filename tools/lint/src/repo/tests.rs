use super::ls_files;
use super::scaffold::Repo;
use std::path::PathBuf;

#[test]
fn lists_what_the_flags_ask_for_relative_to_the_root() {
    let repo = Repo::new("1387-ls", true);
    repo.file("a.rs", "");
    repo.file("b/c.rs", "");
    repo.git(&["add", "a.rs"]);
    let tracked = ls_files(&repo.0, &["--cached"], &[]);
    assert_eq!(tracked, Ok(vec![PathBuf::from("a.rs")]));
    let all = ls_files(&repo.0, &["--cached", "--others"], &["b"]);
    assert_eq!(all, Ok(vec![PathBuf::from("b/c.rs")]));
}

#[cfg(unix)]
#[test]
fn a_non_utf8_name_survives_the_listing() {
    use std::os::unix::ffi::OsStrExt;
    let repo = Repo::new("1387-bytes", true);
    let name = std::ffi::OsStr::from_bytes(b"caf\xe9.rs");
    std::fs::write(repo.0.join(name), "").expect("write");
    let found = ls_files(&repo.0, &["--others"], &[]).expect("git");
    assert_eq!(found, vec![PathBuf::from(name)]);
    assert!(repo.0.join(&found[0]).is_file());
}

#[test]
fn git_s_own_error_comes_back() {
    let repo = Repo::new("1387-err", false);
    let why = ls_files(&repo.0, &[], &[]).expect_err("not a repository");
    assert!(why.contains("not a git repository"), "{why}");
}
