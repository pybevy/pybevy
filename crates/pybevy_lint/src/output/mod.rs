mod diagnostic;
mod formatter;

pub use diagnostic::{Diagnostic, DiagnosticCode, DiagnosticSeverity, Suggestion};
pub use formatter::format_diagnostic;
