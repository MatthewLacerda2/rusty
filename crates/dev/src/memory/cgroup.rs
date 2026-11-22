use std::path::Path;

use super::Machine;

#[cfg_attr(
    all(test, not(target_os = "linux")),
    expect(dead_code, reason = "macOS tests read a fake tree")
)]
pub const ROOT: &str = "/sys/fs/cgroup";

pub fn own_path(proc_cgroup: &str) -> Option<&str> {
    proc_cgroup.lines().find_map(|l| l.strip_prefix("0::"))
}

pub fn bound(root: &Path, path: &str) -> Option<Machine> {
    let own = root.join(path.trim_start_matches('/'));
    let mut bound: Option<Machine> = None;
    for level in own.ancestors().take_while(|l| l.starts_with(root)) {
        let Some(max) = read_number(&level.join("memory.max")) else {
            continue;
        };
        let current = read_number(&level.join("memory.current")).unwrap_or(0);
        let stat = std::fs::read_to_string(level.join("memory.stat")).unwrap_or_default();
        let reclaimable = stat_field(&stat, "inactive_file").unwrap_or(0);
        let here = Machine {
            available: (max.saturating_sub(current) + reclaimable).min(max),
            total: max,
        };
        bound = Some(bound.map_or(here, |b| b.within(here)));
    }
    bound
}

fn read_number(file: &Path) -> Option<u64> {
    std::fs::read_to_string(file).ok()?.trim().parse().ok()
}

fn stat_field(stat: &str, key: &str) -> Option<u64> {
    stat.lines().find_map(|l| {
        let (k, v) = l.split_once(' ')?;
        (k == key).then(|| v.trim().parse().ok())?
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1 << 30;

    fn tree(name: &str, levels: &[(&str, &str, u64, u64)]) -> std::path::PathBuf {
        let root = rusty_test_support::temp::dir().join(format!("rusty_cgroup_{name}"));
        let _ = std::fs::remove_dir_all(&root);
        for &(path, max, current, inactive) in levels {
            let dir = root.join(path);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("memory.max"), format!("{max}\n")).unwrap();
            std::fs::write(dir.join("memory.current"), format!("{current}\n")).unwrap();
            let stat = format!("anon 5\ninactive_file {inactive}\nactive_file 7\n");
            std::fs::write(dir.join("memory.stat"), stat).unwrap();
        }
        root
    }

    #[test]
    fn the_v2_line_names_the_process_s_cgroup() {
        let text = "12:cpu:/old\n0::/system.slice/run-u7.service\n";
        assert_eq!(own_path(text), Some("/system.slice/run-u7.service"));
        assert_eq!(own_path("4:memory:/v1-only\n"), None);
    }

    #[test]
    fn a_memory_max_caps_the_total_and_what_is_free() {
        let ten = (10 * GB).to_string();
        let root = tree(
            "capped",
            &[
                ("", "max", 0, 0),
                ("system.slice", "max", 0, 0),
                ("system.slice/run.service", &ten, 7 * GB, GB),
            ],
        );
        let cap = bound(&root, "/system.slice/run.service").unwrap();
        assert_eq!(
            cap,
            Machine {
                available: 4 * GB,
                total: 10 * GB
            }
        );
        let host = Machine {
            available: 12 * GB,
            total: 16 * GB,
        };
        assert_eq!(host.within(cap), cap, "the cgroup's, not the host's");
    }

    #[test]
    fn the_tightest_ancestor_holds_and_none_capped_is_none() {
        let (eight, twenty) = ((8 * GB).to_string(), (20 * GB).to_string());
        let root = tree(
            "ancestor",
            &[
                ("user.slice", &eight, 6 * GB, 0),
                ("user.slice/job", &twenty, GB, 0),
            ],
        );
        let cap = bound(&root, "/user.slice/job").unwrap();
        assert_eq!(
            cap,
            Machine {
                available: 2 * GB,
                total: 8 * GB
            }
        );
        let open = tree("uncapped", &[("a", "max", GB, 0), ("a/b", "max", GB, 0)]);
        assert_eq!(bound(&open, "/a/b"), None, "\"max\" is no cap");
        assert_eq!(bound(&open, "/not/there"), None);
    }
}
