use std::cmp::Reverse;

use colored::Colorize;

use super::model::TestCoverageReport;
use crate::python_parser::module_symbols::StubSymbolKind;

pub fn format_summary_table(report: &TestCoverageReport, show_members: bool) -> String {
    format_summary_table_detailed(report, show_members, "name", None)
}

pub fn format_summary_table_detailed(
    report: &TestCoverageReport,
    show_members: bool,
    sort: &str,
    max_coverage: Option<f64>,
) -> String {
    let mut output = format!(
        "\n{}\n\n",
        "Public API Exercise Coverage".bold().underline()
    );
    let mut sorted: Vec<_> = report.classes.iter().collect();
    if let Some(max) = max_coverage {
        sorted.retain(|class| f64::from(class.coverage_pct) <= max);
    }
    match sort {
        "name" => sorted.sort_by(|a, b| a.class_path.cmp(&b.class_path)),
        "untested" => {
            sorted.sort_by_key(|class| Reverse(class.unexercised_members.len()));
        }
        _ => sorted.sort_by(|a, b| {
            a.coverage_pct
                .partial_cmp(&b.coverage_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
    }

    let path_width = sorted
        .iter()
        .map(|class| class.class_path.as_str().len())
        .chain(
            report
                .module_symbols
                .iter()
                .map(|symbol| symbol.path.as_str().len()),
        )
        .max()
        .unwrap_or(10)
        .max("Public path".len());
    let total_width = path_width + 2 + 4 + 2 + 9 + 2 + 8;
    output.push_str(&format!(
        "{:<path_width$}  {:>4}  {:>9}  {:>8}\n",
        "Public path".bold(),
        "Ctor".bold(),
        "Operations".bold(),
        "Coverage".bold(),
    ));
    output.push_str(&"-".repeat(total_width));
    output.push('\n');

    for class in &sorted {
        let ctor = if !class.has_constructor {
            "-"
        } else if class.constructor_exercised {
            "yes"
        } else {
            "no"
        };
        let operations = format!("{}/{}", class.exercised_members.len(), class.total_members);
        output.push_str(&format!(
            "{:<path_width$}  {:>4}  {:>9}  {:>7.0}%\n",
            class.class_path, ctor, operations, class.coverage_pct,
        ));
        if show_members {
            if !class.evidence_files.is_empty() {
                output.push_str(&format!(
                    "  evidence: {}\n",
                    class
                        .evidence_files
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            for member in &class.unexercised_members {
                output.push_str(&format!("  - {}.{}\n", class.class_path, member));
            }
        }
    }

    output.push_str(&"-".repeat(total_width));
    output.push_str("\n\nModule symbols\n\n");
    output.push_str(&format!(
        "{:<path_width$}  {:<8}  {:<10}  {:<8}  {}\n",
        "Public path", "Kind", "Referenced", "Executed", "Asserted"
    ));
    output.push_str(&"-".repeat(total_width));
    output.push('\n');
    for symbol in &report.module_symbols {
        output.push_str(&format!(
            "{:<path_width$}  {:<8}  {:<10}  {:<8}  {}\n",
            symbol.path,
            symbol_kind(symbol.kind),
            if symbol.is_exercised { "yes" } else { "no" },
            evidence_word(report.execution_data_loaded, symbol.is_executed),
            evidence_word(report.execution_data_loaded, symbol.is_asserted),
        ));
        if show_members && !symbol.evidence_files.is_empty() {
            output.push_str(&format!(
                "  evidence: {}\n",
                symbol
                    .evidence_files
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    output.push_str(&"-".repeat(total_width));
    output.push('\n');
    if max_coverage.is_some() {
        output.push_str(&format!("Showing {} classes (filtered). ", sorted.len()));
    }
    output.push_str(&format!(
        "Referenced: {}/{} classes ({:.0}%), {}/{} operations ({:.0}%)\n",
        report.exercised_classes,
        report.total_classes,
        report.class_coverage_pct(),
        report.exercised_members,
        report.total_members,
        report.member_coverage_pct(),
    ));
    if report.execution_data_loaded {
        output.push_str(&format!(
            "Executed: {}/{} operations; behavior-asserted: {}/{} operations\n",
            report.executed_members,
            report.total_members,
            report.asserted_members,
            report.total_members,
        ));
    } else {
        output.push_str(
            "Executed: not measured; behavior-asserted: not measured (pass --execution-report)\n",
        );
    }
    output.push_str(&format!(
        "Unresolved PyBevy references: {}\n",
        report.unresolved.len()
    ));
    output.push_str(&format!(
        "Module symbols: {}/{} exercised\n",
        report.exercised_module_symbols, report.total_module_symbols
    ));
    if show_members {
        for reference in &report.unresolved {
            output.push_str(&format!(
                "  - {}:{}: `{}` ({})\n",
                reference.file.display(),
                reference.line,
                reference.expression,
                reference.reason,
            ));
        }
    }
    output
}

fn evidence_word(available: bool, present: bool) -> &'static str {
    if !available {
        "unknown"
    } else if present {
        "yes"
    } else {
        "no"
    }
}

fn symbol_kind(kind: StubSymbolKind) -> &'static str {
    match kind {
        StubSymbolKind::Function => "function",
        StubSymbolKind::Alias => "alias",
        StubSymbolKind::Constant => "constant",
        StubSymbolKind::Class => "class",
    }
}
