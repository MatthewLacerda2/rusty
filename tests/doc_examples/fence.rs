//! Find the ```lua blocks in a Markdown document and read their fence markers.

/// What a block's fence asks of the test.
#[derive(Debug, PartialEq)]
pub enum Mode {
    /// Plain ```lua: compiled and run in a fresh headless session.
    Run,
    /// ```lua no-run: compiled only (it writes files, calls a provider, or leans on
    /// game-side helpers the doc doesn't define).
    NoRun,
    /// ```lua ignore: not Lua at all (a shape sketch with `|` or `...`); skipped.
    Ignore,
}

/// One ```lua block: where it starts, how to treat it, and its source.
#[derive(Debug)]
pub struct Block {
    /// 1-based line of the opening fence.
    pub line: usize,
    pub mode: Mode,
    /// `with=name,name:Component+Component`: entities the block names but doesn't
    /// create — each one is made fresh, bound to a global of that name, and given
    /// the listed components before the block runs.
    pub with: Vec<(String, Vec<String>)>,
    pub source: String,
}

/// Every ```lua block in `doc`, in order. A fence may be indented (a block inside
/// a list item); its body loses the fence's indentation. Other fences are stepped
/// over whole, so a ```lua quoted inside one is not a block.
pub fn lua_blocks(doc: &str) -> Result<Vec<Block>, String> {
    let mut blocks = Vec::new();
    let mut in_other = false;
    let mut open: Option<(usize, Block)> = None;
    for (i, raw) in doc.lines().enumerate() {
        let indent = raw.len() - raw.trim_start().len();
        let line = raw.trim_start();
        let fence = line.strip_prefix("```");
        // Only a bare ``` closes a fence (CommonMark); ```lua inside one is text.
        let closes = line.trim_end() == "```";
        if let Some((pad, block)) = open.as_mut() {
            if closes {
                blocks.push(open.take().expect("open").1);
            } else {
                block
                    .source
                    .push_str(raw.get(indent.min(*pad)..).unwrap_or(""));
                block.source.push('\n');
            }
        } else if in_other {
            in_other = !closes;
        } else if let Some(info) = fence {
            let mut words = info.split_whitespace();
            if words.next() == Some("lua") {
                open = Some((indent, header(i + 1, words)?));
            } else {
                in_other = true;
            }
        }
    }
    Ok(blocks)
}

/// Parse the words after ```lua into a block header, refusing unknown markers.
fn header<'a>(line: usize, words: impl Iterator<Item = &'a str>) -> Result<Block, String> {
    let mut block = Block {
        line,
        mode: Mode::Run,
        with: Vec::new(),
        source: String::new(),
    };
    for word in words {
        match word {
            "no-run" => block.mode = Mode::NoRun,
            "ignore" => block.mode = Mode::Ignore,
            _ => {
                let names = word
                    .strip_prefix("with=")
                    .ok_or_else(|| format!("line {line}: unknown fence marker `{word}`"))?;
                for spec in names.split(',') {
                    let (name, comps) = spec.split_once(':').unwrap_or((spec, ""));
                    let comps = comps.split('+').filter(|c| !c.is_empty());
                    block
                        .with
                        .push((name.to_string(), comps.map(String::from).collect()));
                }
            }
        }
    }
    Ok(block)
}
