use std::collections::BTreeMap;

pub const OWN_FLOOR: f64 = 0.20;
pub const COUNT_FLOOR: f64 = 0.10;
pub const UNKNOWN: &str = "[unknown]";

pub const REFUSED: &str = "this profile is not to be trusted:";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tally {
    pub samples: u64,
    pub own: u64,
    pub unknown: u64,
    pub images: BTreeMap<String, u64>,
    pub warnings: Vec<String>,
}

pub fn image_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn frame_name(symbol: &str, image: &str) -> String {
    if !symbol.is_empty() && symbol != UNKNOWN {
        return symbol.to_string();
    }
    match image_name(image) {
        "" => UNKNOWN.to_string(),
        bracketed if bracketed.starts_with('[') => bracketed.to_string(),
        name => format!("[{name}]"),
    }
}

pub fn refusal_gist(text: &str) -> Option<String> {
    let (_, after) = text.split_once(REFUSED)?;
    let first = after
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("- "))?;
    let words: Vec<&str> = first.split_whitespace().take(8).collect();
    Some(format!("{} ...", words.join(" ")))
}

pub fn own_image() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    let name = exe.file_name().map(|n| n.to_string_lossy().into_owned());
    name.unwrap_or_default()
}

impl Tally {
    pub fn count(&mut self, image: &str, symbol: &str, own: &str, n: u64) {
        let image = image_name(image);
        let unknown = symbol.is_empty() || symbol == UNKNOWN;
        self.samples += n;
        self.own += u64::from(!unknown && image == own) * n;
        self.unknown += u64::from(unknown) * n;
        let key = if image.is_empty() { UNKNOWN } else { image };
        *self.images.entry(key.to_string()).or_default() += n;
    }

    pub fn merge(&mut self, other: Tally) {
        self.samples += other.samples;
        self.own += other.own;
        self.unknown += other.unknown;
        for (image, n) in other.images {
            *self.images.entry(image).or_default() += n;
        }
        self.warnings.extend(other.warnings);
    }

    fn share(&self, n: u64) -> f64 {
        n as f64 / self.samples.max(1) as f64 * 100.0
    }

    pub fn images_line(&self) -> String {
        let mut rows: Vec<(&String, &u64)> = self.images.iter().collect();
        rows.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let named: Vec<String> = (rows.iter().take(5))
            .map(|(image, &n)| format!("{:.1}% {image}", self.share(n)))
            .collect();
        named.join(", ")
    }
}

pub fn refusal(tally: &Tally, expected: f64) -> Option<String> {
    let mut why = Vec::new();
    let own = tally.share(tally.own);
    if own < OWN_FLOOR * 100.0 {
        why.push(format!(
            "only {own:.1}% of the samples resolve to a function of the bench's own \
             binary (the floor is {:.0}%); {:.1}% have no symbol at all",
            OWN_FLOOR * 100.0,
            tally.share(tally.unknown),
        ));
    }
    if (tally.samples as f64) < expected * COUNT_FLOOR {
        why.push(format!(
            "too few samples: {} where the measured frames should give about {expected:.0} \
             (the floor is a tenth), so most were lost or never taken",
            tally.samples
        ));
    }
    if why.is_empty() {
        return None;
    }
    let mut out = String::from(REFUSED);
    for reason in why.iter().chain(&tally.warnings) {
        out.push_str(&format!("\n  - {reason}"));
    }
    out.push_str(&format!(
        "\n  where the samples landed: {}",
        tally.images_line()
    ));
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: &str = "bench-scenes";

    fn tally(own: u64, unknown: u64, libc: u64) -> Tally {
        let mut t = Tally::default();
        t.count(
            "/repo/target/profiling/bench-scenes",
            "rusty_dev::bench::measure_posed",
            OWN,
            own,
        );
        t.count("/repo/target/profiling/bench-scenes", UNKNOWN, OWN, unknown);
        t.count("/usr/lib/libc.so.6", "pthread_cond_timedwait", OWN, libc);
        t
    }

    #[test]
    fn a_mostly_unknown_profile_is_refused_with_its_numbers() {
        let t = tally(50, 1580, 317);
        assert_eq!((t.samples, t.own, t.unknown), (1947, 50, 1580));
        let said = refusal(&t, 2000.0).expect("refused");
        assert!(
            said.starts_with("this profile is not to be trusted:"),
            "{said}"
        );
        assert!(said.contains("only 2.6% of the samples"), "{said}");
        assert!(said.contains("81.2% have no symbol"), "{said}");
        assert!(
            said.contains("83.7% bench-scenes, 16.3% libc.so.6"),
            "{said}"
        );
        assert!(!said.contains("too few"), "{said}");
        let gist = refusal_gist(&format!("head\n{said}\n")).expect("a gist");
        assert_eq!(gist, "only 2.6% of the samples resolve to a ...");
        assert_eq!(refusal_gist("head\nwhere the samples landed: x"), None);
    }

    #[test]
    fn an_unnamed_frame_is_named_by_its_image() {
        assert_eq!(frame_name("main", "/bin/bench-scenes"), "main");
        assert_eq!(frame_name(UNKNOWN, "/usr/lib/libc.so.6"), "[libc.so.6]");
        assert_eq!(
            frame_name("", "libnvidia-eglcore.so.550"),
            "[libnvidia-eglcore.so.550]"
        );
        assert_eq!(frame_name(UNKNOWN, "[unknown]"), UNKNOWN);
        assert_eq!(frame_name(UNKNOWN, "[vdso]"), "[vdso]");
        assert_eq!(frame_name("", ""), UNKNOWN);
    }

    #[test]
    fn too_few_samples_are_refused_and_a_sound_profile_is_not() {
        let mut few = tally(60, 10, 30);
        few.warnings.push("perf: lost 3 chunks".into());
        let said = refusal(&few, 15_000.0).expect("refused");
        assert!(said.contains("too few samples: 100 where"), "{said}");
        assert!(said.contains("about 15000"), "{said}");
        assert!(said.contains("- perf: lost 3 chunks"), "{said}");
        assert!(!said.contains("only"), "{said}");
        assert_eq!(refusal(&tally(580, 20, 400), 1500.0), None);
        assert_eq!(
            refusal(&Tally::default(), 0.0).map(|s| s.contains("0.0%")),
            Some(true)
        );
    }
}
