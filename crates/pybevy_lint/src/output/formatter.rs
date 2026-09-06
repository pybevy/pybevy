use colored::Colorize;

use super::{Diagnostic, DiagnosticSeverity};

/// Format a diagnostic for terminal output
pub fn format_diagnostic(diag: &Diagnostic) -> String {
    let mut output = String::new();

    // Header: error[E001]: message
    let severity_str = match diag.severity {
        DiagnosticSeverity::Error => "error".red().bold(),
        DiagnosticSeverity::Warning => "warning".yellow().bold(),
        DiagnosticSeverity::Info => "info".blue().bold(),
    };

    output.push_str(&format!(
        "{}[{}]: {}\n",
        severity_str,
        diag.code.as_str(),
        diag.message
    ));

    // Primary location
    if let Some(ref loc) = diag.primary_location {
        output.push_str(&format!(
            "  {} {}:{}:{}\n",
            "-->".blue(),
            loc.file.display(),
            loc.line,
            loc.column
        ));
    }

    // Secondary locations
    for (loc, label) in &diag.secondary_locations {
        output.push_str(&format!(
            "  {} {}:{}:{}: {}\n",
            ":".blue(),
            loc.file.display(),
            loc.line,
            loc.column,
            label
        ));
    }

    // Notes
    for note in &diag.notes {
        output.push_str(&format!("  {} {}: {}\n", "=".blue(), "note".bold(), note));
    }

    // Suggestions
    for suggestion in &diag.suggestions {
        output.push_str(&format!(
            "  {} {}: {}\n",
            "=".blue(),
            "help".bold(),
            suggestion.message
        ));
        if let Some(ref replacement) = suggestion.replacement {
            output.push_str(&format!("          {}\n", replacement.green()));
        }
    }

    output
}
