use super::{split, Kind};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub const FLAG: &str = "--comment-signals";

/// Two sentences this alike in their words (shared over all) are one point said twice.
const ALIKE: f64 = 0.8;
/// Shorter sentences repeat by chance ("Off by default.").
const MIN_WORDS: usize = 6;
const OPENERS: &[&str] = &["so", "on", "in", "of", "not", "before"];
/// Verbs a fragment's opening clause lacks; a word ending in `s` or `ed` counts as one too.
const VERBS: &[&str] = &[
    "is", "are", "was", "were", "be", "been", "has", "have", "had", "do", "does", "did", "can",
    "could", "must", "may", "might", "will", "would", "should", "shall", "keep", "run", "take",
    "make", "read", "write", "draw", "go", "come", "see", "get", "set",
];

pub struct Sentence {
    pub path: String,
    pub line: usize,
    words: BTreeSet<String>,
}

impl Sentence {
    fn new(path: &str, line: usize, text: &str) -> Sentence {
        let words = super::header::words(text).into_iter().collect();
        Sentence {
            path: path.to_string(),
            line,
            words,
        }
    }

    fn alike(&self, other: &Sentence) -> bool {
        let shared = self.words.intersection(&other.words).count() as f64;
        let all = self.words.union(&other.words).count() as f64;
        all > 0.0 && shared / all >= ALIKE
    }
}

/// The sentences of `pool`, sorted by word count, alike `a`.
fn alike<'a>(a: &'a Sentence, pool: &'a [&'a Sentence]) -> impl Iterator<Item = &'a Sentence> {
    let n = a.words.len() as f64;
    let from = pool.partition_point(|s| (s.words.len() as f64) < n * ALIKE);
    let to = pool.partition_point(|s| s.words.len() as f64 <= n / ALIKE);
    pool[from..to.max(from)]
        .iter()
        .copied()
        .filter(move |b| a.alike(b))
}

fn blocks(kind: Kind, source: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut open = false;
    for (i, line) in split(kind, source).iter().enumerate() {
        let text = super::bare(&line.comment).trim();
        let note = line.code.trim().is_empty() && !text.is_empty() && !super::marked(text);
        match out.last_mut().filter(|_| open && note) {
            Some((_, block)) => {
                block.push(' ');
                block.push_str(text);
            }
            None if note => out.push((i + 1, text.to_string())),
            None => {}
        }
        open = note;
    }
    out
}

fn sentences(path: &str, line: usize, block: &str) -> Vec<Sentence> {
    let pieces = block.split_inclusive(". ").map(str::trim);
    let kept = pieces.map(|s| Sentence::new(path, line, s));
    kept.filter(|s| s.words.len() >= MIN_WORDS).collect()
}

/// A block opening on a preposition or `So`/`Not` whose first sentence has no verb.
fn fragment(block: &str) -> bool {
    let first = block.split('.').next().unwrap_or("");
    let words = super::header::words(first);
    let opens = words.first().is_some_and(|w| OPENERS.contains(&w.as_str()));
    let verb = |w: &String| VERBS.contains(&w.as_str()) || w.ends_with("ed") || w.ends_with('s');
    opens && !words.iter().skip(1).any(verb)
}

fn same_crate(a: &str, b: &str) -> bool {
    let root = |p: &str| {
        crate::modules::crate_module(p).map_or_else(
            || p.split('/').next().unwrap_or("").to_string(),
            str::to_string,
        )
    };
    root(a) == root(b)
}

pub fn report(base: &Path, files: &[(String, String)]) -> String {
    let mut said: Vec<Sentence> = Vec::new();
    let mut out = String::new();
    for (path, source) in files {
        let Some(kind) = Kind::of(path) else { continue };
        for (line, block) in blocks(kind, source) {
            if fragment(&block) {
                out.push_str(&format!(
                    "{path}:{line}: [comment-fragment] opens on a fragment; start with \
                     what the code does\n"
                ));
            }
            said.extend(sentences(path, line, &block));
        }
    }
    let mut pool: Vec<&Sentence> = said.iter().collect();
    pool.sort_by_key(|s| s.words.len());
    for a in &said {
        let twin = alike(a, &pool).find(|b| {
            let other = (&b.path, b.line) > (&a.path, a.line);
            other && same_crate(&a.path, &b.path)
        });
        if let Some(b) = twin {
            out.push_str(&format!(
                "{}:{}: [comment-duplicate] also said at {}:{}: keep one site\n",
                a.path, a.line, b.path, b.line
            ));
        }
    }
    let docs = doc_sentences(base);
    let mut pages: Vec<&Sentence> = docs.iter().collect();
    pages.sort_by_key(|s| s.words.len());
    for s in &said {
        if let Some(doc) = alike(s, &pages).next() {
            out.push_str(&format!(
                "{}:{}: [comment-doc] restates {}:{}: point at the doc instead\n",
                s.path, s.line, doc.path, doc.line
            ));
        }
    }
    format!("signal (not a failure): comments\n{out}")
}

/// The signals over the files git lists under `paths`; it reports, never fails.
pub fn run(base: &Path, paths: &[String]) -> String {
    if paths.is_empty() {
        return format!(
            "lint: {FLAG} takes the paths to read (make comments-signal PATHS=crates/ui)\n"
        );
    }
    let specs: Vec<&str> = paths.iter().map(String::as_str).collect();
    let flags = ["--cached", "--others", "--exclude-standard"];
    let listed = crate::repo::ls_files(base, &flags, &specs).unwrap_or_default();
    let files: Vec<(String, String)> = listed
        .iter()
        .filter_map(|p| {
            let text = fs::read_to_string(base.join(p)).ok()?;
            Some((p.to_string_lossy().into_owned(), text))
        })
        .collect();
    report(base, &files)
}

fn doc_sentences(base: &Path) -> Vec<Sentence> {
    let mut found = Vec::new();
    let mut pages: BTreeMap<String, String> = BTreeMap::new();
    collect(&base.join("docs"), base, &mut pages);
    for (path, text) in &pages {
        for (i, line) in text.lines().enumerate() {
            found.extend(sentences(path, i + 1, line));
        }
    }
    found
}

fn collect(dir: &Path, base: &Path, out: &mut BTreeMap<String, String>) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, base, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            let rel = path.strip_prefix(base).unwrap_or(&path);
            let text = fs::read_to_string(&path).unwrap_or_default();
            out.insert(rel.to_string_lossy().into_owned(), text);
        }
    }
}

#[cfg(test)]
#[path = "signals_tests.rs"]
mod tests;
