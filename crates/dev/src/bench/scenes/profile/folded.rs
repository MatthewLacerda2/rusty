//! Folded stacks (`root;caller;leaf count`, one line per distinct stack, the
//! format flame-graph tools read) and the two tables a profile prints from them:
//! the functions by self time (samples where the function was the leaf) and by
//! total time (samples where it was anywhere on the stack, once per sample even
//! when it recursed).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

/// How many functions each table lists.
pub const TOP: usize = 40;

/// Samples by stack, root first, frames joined with `;`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Folded(pub BTreeMap<String, u64>);

impl Folded {
    /// Count one sample of `frames`, root first.
    pub fn add(&mut self, frames: &[String], count: u64) {
        if frames.is_empty() || count == 0 {
            return;
        }
        // A `;` inside a name would split it: folded stacks have no escape.
        let clean: Vec<String> = frames.iter().map(|f| f.replace(';', ",")).collect();
        *self.0.entry(clean.join(";")).or_default() += count;
    }

    /// Add every stack of `other`.
    pub fn merge(&mut self, other: Folded) {
        for (stack, n) in other.0 {
            *self.0.entry(stack).or_default() += n;
        }
    }

    pub fn samples(&self) -> u64 {
        self.0.values().sum()
    }

    /// The stacks as text, one `stack count` line each.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (stack, n) in &self.0 {
            let _ = writeln!(out, "{stack} {n}");
        }
        out
    }

    /// The functions with the most samples as the leaf, and anywhere on the
    /// stack: at most [`TOP`] each, most first, ties by name.
    pub fn tops(&self) -> (Vec<(String, u64)>, Vec<(String, u64)>) {
        let mut own: BTreeMap<&str, u64> = BTreeMap::new();
        let mut total: BTreeMap<&str, u64> = BTreeMap::new();
        for (stack, &n) in &self.0 {
            let frames: Vec<&str> = stack.split(';').collect();
            if let Some(leaf) = frames.last() {
                *own.entry(leaf).or_default() += n;
            }
            for f in frames.into_iter().collect::<BTreeSet<_>>() {
                *total.entry(f).or_default() += n;
            }
        }
        (ranked(own), ranked(total))
    }

    /// The profile's text: a header line `head`, then both tables.
    pub fn tables(&self, head: &str) -> String {
        let samples = self.samples().max(1) as f64;
        let (own, total) = self.tops();
        let mut out = format!("{head}\n");
        for (title, rows) in [("self", own), ("total", total)] {
            let _ = writeln!(
                out,
                "\nby {title} time (top {TOP}):\n{:>7} {:>9}  function",
                "%", "samples"
            );
            for (name, n) in rows {
                let _ = writeln!(out, "{:>6.2}% {n:>9}  {name}", n as f64 / samples * 100.0);
            }
        }
        out
    }
}

fn ranked(counts: BTreeMap<&str, u64>) -> Vec<(String, u64)> {
    let mut rows: Vec<(String, u64)> = counts
        .into_iter()
        .map(|(k, n)| (k.to_string(), n))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.truncate(TOP);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(s: &str) -> Vec<String> {
        s.split(';').map(String::from).collect()
    }

    #[test]
    fn self_counts_the_leaf_and_total_counts_each_function_once_per_sample() {
        let mut f = Folded::default();
        f.add(&frames("main;frame;draw"), 3);
        f.add(&frames("main;frame;step"), 1);
        f.add(&frames("main;frame;draw"), 1);
        // Recursion counts once in total.
        f.add(&frames("main;walk;walk"), 2);
        let (own, total) = f.tops();
        assert_eq!(own[0], ("draw".to_string(), 4));
        assert_eq!(own[1], ("walk".to_string(), 2));
        assert_eq!(total[0], ("main".to_string(), 7));
        assert!(total.contains(&("walk".to_string(), 2)), "{total:?}");
        assert_eq!(f.samples(), 7);
        assert!(f.text().contains("main;frame;draw 4\n"), "{}", f.text());
        let text = f.tables("profile");
        assert!(text.contains("by self time") && text.contains("by total time"));
        assert!(text.contains(" 57.14%         4  draw"), "{text}");
    }
}
