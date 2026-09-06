use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result};
use serde::Deserialize;

use super::model::TestUsage;

#[derive(Debug, Deserialize)]
struct CoverageReport {
    files: HashMap<String, CoverageFile>,
}

#[derive(Debug, Deserialize)]
struct CoverageFile {
    executed_lines: HashSet<usize>,
}

/// Runtime line evidence exported by `coverage json`.
///
/// A covered line proves that Python reached the source line. It does not by
/// itself prove that every expression on a compound or short-circuiting line
/// ran, so assertion evidence is accepted only for API references located in
/// an assertion/expected-error body and on a covered line.
#[derive(Debug)]
pub struct ExecutionReport {
    files: HashMap<String, HashSet<usize>>,
}

impl ExecutionReport {
    pub fn read(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("Failed to read execution report {}", path.display()))?;
        let report: CoverageReport = serde_json::from_str(&raw)
            .with_context(|| format!("Failed to parse execution report {}", path.display()))?;
        Ok(Self {
            files: report
                .files
                .into_iter()
                .map(|(path, file)| (normalize_path(&path), file.executed_lines))
                .collect(),
        })
    }

    fn contains(&self, file: &Path, line: usize) -> bool {
        let evidence = normalize_path(&file.to_string_lossy());
        self.files
            .iter()
            .any(|(reported, lines)| paths_match(reported, &evidence) && lines.contains(&line))
    }
}

pub fn apply_execution_report(usage: &mut TestUsage, report: &ExecutionReport) {
    usage.execution_data_loaded = true;
    for class in usage.classes.values_mut() {
        for (member, sites) in &class.member_sites {
            if sites
                .iter()
                .any(|site| site.execution_eligible && report.contains(&site.file, site.line))
            {
                class.executed_members.insert(member.clone());
            }
        }
        for (member, sites) in &class.assertion_sites {
            if sites
                .iter()
                .any(|site| site.execution_eligible && report.contains(&site.file, site.line))
            {
                class.asserted_members.insert(member.clone());
            }
        }
    }
    for symbol in usage.symbols.values_mut() {
        if symbol
            .sites
            .iter()
            .any(|site| site.execution_eligible && report.contains(&site.file, site.line))
        {
            symbol.executed_kinds.extend(symbol.kinds.iter().copied());
        }
        if symbol
            .assertion_sites
            .iter()
            .any(|site| site.execution_eligible && report.contains(&site.file, site.line))
        {
            symbol.asserted_kinds.extend(symbol.kinds.iter().copied());
        }
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string()
}

fn paths_match(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_suffix(right)
            .is_some_and(|prefix| prefix.ends_with('/'))
        || right
            .strip_suffix(left)
            .is_some_and(|prefix| prefix.ends_with('/'))
}
