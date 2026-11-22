use std::fmt::Write as _;

/// One blessed conversion.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub converter: String,
    pub version: u32,
    pub fixture: String,
    /// The hash of the conversion's output, 16 hex digits.
    pub hash: String,
    /// The program's version ([`Converter::tool`](crate::convert::Converter::tool)),
    /// `-` for a converter inside the engine. Last, since it may hold spaces.
    pub tool: String,
}

pub struct Now {
    pub converter: String,
    pub version: u32,
    pub tool: String,
    pub hashes: Vec<(String, String)>,
}

impl Now {
    fn rows(&self) -> impl Iterator<Item = Row> + '_ {
        self.hashes.iter().map(|(fixture, hash)| Row {
            converter: self.converter.clone(),
            version: self.version,
            fixture: fixture.clone(),
            hash: hash.clone(),
            tool: self.tool.clone(),
        })
    }
}

const HEADER: &str = "\
# The converters' output, pinned (#1286): converter version fixture hash tool.
# Written by `make bless-converters`, never by hand (docs/testing.md).
";

/// The rows of a table file; blank lines and `#` comments skipped.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let rows = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    rows.map(|line| {
        let mut fields = line.splitn(5, ' ');
        let mut next = || fields.next().filter(|f| !f.is_empty());
        let bad = || format!("a malformed fingerprint row: {line:?}");
        let (converter, version, fixture, hash, tool) = (next(), next(), next(), next(), next());
        Ok(Row {
            converter: converter.ok_or_else(bad)?.to_string(),
            version: version.and_then(|v| v.parse().ok()).ok_or_else(bad)?,
            fixture: fixture.ok_or_else(bad)?.to_string(),
            hash: hash.ok_or_else(bad)?.to_string(),
            tool: tool.ok_or_else(bad)?.to_string(),
        })
    })
    .collect()
}

/// Sorted, so a re-bless changes only what changed.
pub fn format(rows: &[Row]) -> String {
    let mut rows = rows.to_vec();
    rows.sort_by(|a, b| (&a.converter, &a.fixture).cmp(&(&b.converter, &b.fixture)));
    let mut text = HEADER.to_string();
    for r in &rows {
        let (c, v, f, h, t) = (&r.converter, r.version, &r.fixture, &r.hash, &r.tool);
        writeln!(text, "{c} {v} {f} {h} {t}").expect("a String takes writes");
    }
    text
}

pub fn check(table: &[Row], now: &Now) -> Result<(), String> {
    let name = &now.converter;
    let blessed: Vec<&Row> = table.iter().filter(|r| &r.converter == name).collect();
    let rebless = "re-bless the table with `make bless-converters`";
    let Some(first) = blessed.first() else {
        return Err(format!(
            "the {name} converter has no fingerprint: {rebless}"
        ));
    };
    if first.version != now.version {
        return Err(format!(
            "the {name} converter is at version {} but its fingerprint was blessed at {}: {rebless}",
            now.version, first.version
        ));
    }
    if first.tool != now.tool {
        return Err(format!(
            "the {name} converter's fingerprint was blessed with its program at {}, this one is {}: {rebless}",
            first.tool, now.tool
        ));
    }
    let blessed_hash = |fixture: &str| blessed.iter().find(|b| b.fixture == fixture);
    let changed: Vec<&str> = now
        .hashes
        .iter()
        .filter(|(f, h)| blessed_hash(f).is_some_and(|b| &b.hash != h))
        .map(|(f, _)| f.as_str())
        .collect();
    if !changed.is_empty() {
        return Err(format!(
            "the {name} converter's output changed ({}): bump its `version` in \
             `convert/converter.rs` and {rebless}",
            changed.join(", ")
        ));
    }
    let added = now.hashes.iter().any(|(f, _)| blessed_hash(f).is_none());
    if added || blessed.len() != now.hashes.len() {
        return Err(format!(
            "the {name} converter's fixtures changed: {rebless}"
        ));
    }
    Ok(())
}

/// Refused when the output changed at the blessed version and program: re-blessing
/// never stands in for the bump.
pub fn bless(table: &[Row], now: &Now) -> Result<Vec<Row>, String> {
    let name = &now.converter;
    let same = |r: &&Row| &r.converter == name && r.version == now.version && r.tool == now.tool;
    for old in table.iter().filter(same) {
        if let Some(row) = now
            .rows()
            .find(|n| n.fixture == old.fixture && n.hash != old.hash)
        {
            return Err(format!(
                "the {name} converter's output changed ({}) at version {}: bump its \
                 `version` in `convert/converter.rs` before re-blessing",
                row.fixture, now.version
            ));
        }
    }
    let mut rows: Vec<Row> = table
        .iter()
        .filter(|r| &r.converter != name)
        .cloned()
        .collect();
    rows.extend(now.rows());
    Ok(rows)
}
