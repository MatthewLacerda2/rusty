//! The recent-projects list (#854): order, cap, persistence and where it lives.

use super::*;

fn fresh(name: &str) -> PathBuf {
    let dir = crate::test_temp::dir().join("recent_projects").join(name);
    std::fs::remove_dir_all(&dir).ok();
    dir
}

fn paths(list: &RecentProjects) -> Vec<&Path> {
    list.projects.iter().map(|p| p.path.as_path()).collect()
}

#[test]
fn opening_a_project_moves_it_to_the_top_once() {
    let mut list = RecentProjects::default();
    list.touch(Path::new("/a"), 1);
    list.touch(Path::new("/b"), 2);
    list.touch(Path::new("/a"), 3);
    assert_eq!(paths(&list), [Path::new("/a"), Path::new("/b")]);
    assert_eq!(list.projects[0].opened_at, 3);
    list.remove(Path::new("/a"));
    assert_eq!(paths(&list), [Path::new("/b")]);
}

#[test]
fn the_list_keeps_only_the_most_recent() {
    let mut list = RecentProjects::default();
    for i in 0..(MAX_RECENT as u64 + 5) {
        list.touch(Path::new(&format!("/p{i}")), i);
    }
    assert_eq!(list.projects.len(), MAX_RECENT);
    assert_eq!(
        list.projects[0].path,
        Path::new(&format!("/p{}", MAX_RECENT + 4))
    );
}

#[test]
fn the_list_round_trips_and_a_broken_file_is_empty() {
    let dir = fresh("round_trip");
    let path = dir.join("nested").join(RECENT_FILE);
    assert_eq!(RecentProjects::load(&path), RecentProjects::default());
    let mut list = RecentProjects::default();
    list.touch(&dir, 42);
    list.save(&path).unwrap();
    assert_eq!(RecentProjects::load(&path), list);
    std::fs::write(&path, "{ not json").unwrap();
    assert_eq!(RecentProjects::load(&path), RecentProjects::default());
}

#[test]
fn a_missing_folder_is_flagged_and_named_by_its_last_component() {
    let dir = fresh("missing");
    let entry = RecentProject {
        path: dir.join("horde"),
        opened_at: 0,
    };
    assert_eq!(entry.name(), "horde");
    assert!(entry.is_missing());
    std::fs::create_dir_all(&entry.path).unwrap();
    assert!(!entry.is_missing());
}

#[test]
fn the_config_dir_follows_the_platform() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            let hit = pairs.iter().find(|(k, _)| *k == name);
            hit.map(|(_, v)| v.to_string())
        }
    };
    assert_eq!(config_dir(env(&[])), None, "no home, no list");
    let home = config_dir(env(&[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "")]));
    if cfg!(target_os = "macos") {
        assert_eq!(
            home.unwrap(),
            Path::new("/home/u/Library/Application Support/rusty")
        );
    } else {
        assert_eq!(home.unwrap(), Path::new("/home/u/.config/rusty"));
        let xdg = config_dir(env(&[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "/x")]));
        assert_eq!(xdg.unwrap(), Path::new("/x/rusty"));
    }
}

#[test]
fn ages_read_like_the_hub() {
    assert_eq!(ago(100, 100), "just now");
    assert_eq!(ago(100, 159), "just now");
    assert_eq!(ago(0, 60), "1 minute ago");
    assert_eq!(ago(0, 5 * 3_600), "5 hours ago");
    assert_eq!(ago(0, 3 * 86_400), "3 days ago");
    assert_eq!(ago(0, 400 * 86_400), "1 year ago");
    assert_eq!(ago(10, 0), "just now", "a clock that went back");
}
