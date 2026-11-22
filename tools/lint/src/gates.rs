use std::fs;
use std::path::Path;
use std::process::exit;

const REPORT: &str = ".lint/report.txt";

pub struct Verdict {
    pub findings: Vec<String>,
    pub body: String,
    pub advice: String,
}

impl Verdict {
    pub fn listed(name: &str, findings: Vec<String>, advice: impl Fn(usize) -> String) -> Self {
        let head = if findings.is_empty() { "ok" } else { "FAILED" };
        let mut body = format!("{name}: {head}\n");
        for f in &findings {
            body.push_str(f);
            body.push('\n');
        }
        let advice = if findings.is_empty() {
            String::new()
        } else {
            advice(findings.len())
        };
        Verdict {
            findings,
            body,
            advice,
        }
    }
}

pub type Check = fn(&Path, &[String]) -> Verdict;

pub const GATES: &[(Check, &str)] = &[
    // sorted: gates are independent; `main` runs the one its flag names
    (crate::components::verdict, "--components"),
    (crate::determinism::verdict, "--determinism"),
    (crate::layers::verdict, "--layers"),
    (crate::parity::verdict, "--parity"),
    (crate::rules::verdict, "--rules"),
    (crate::size::verdict, ""),
];

pub const DEAD_PUB: &str = "--dead-pub";

pub fn select(args: &[String]) -> Result<Check, String> {
    let named: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| {
            *a == DEAD_PUB
                || *a == crate::comments::proof::FLAG
                || GATES.iter().any(|(_, f)| !f.is_empty() && f == a)
        })
        .collect();
    if let [_, _, ..] = named.as_slice() {
        return Err(format!(
            "lint: {} name more than one gate; run one gate per call",
            named.join(" ")
        ));
    }
    let flag = named.first().copied().unwrap_or("");
    Ok(GATES
        .iter()
        .find(|(_, f)| *f == flag)
        .map_or(crate::size::verdict, |(check, _)| *check))
}

pub fn report(verdict: &Verdict) {
    fs::create_dir_all(".lint").ok();
    fs::write(REPORT, &verdict.body).ok();
    if verdict.findings.is_empty() {
        print!("{}", verdict.body);
        return;
    }
    eprint!("{}", verdict.body);
    eprint!("{}", verdict.advice);
    exit(1);
}

#[cfg(test)]
#[path = "gates_tests.rs"]
mod tests;
