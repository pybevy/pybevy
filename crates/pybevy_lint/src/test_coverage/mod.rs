pub mod analyzer;
pub mod baseline;
pub mod execution;
pub mod model;
pub mod report;
pub mod test_parser;

pub use analyzer::{analyze_coverage, generate_diagnostics};
pub use baseline::{check_baseline, format_debt_summary, update_baseline};
pub use execution::{ExecutionReport, apply_execution_report};
pub use model::{ApiCatalog, ApiPath, TestCoverageReport, TestUsage};
pub use report::{format_summary_table, format_summary_table_detailed};
pub use test_parser::parse_test_files;
