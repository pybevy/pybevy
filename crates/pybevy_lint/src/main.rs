use std::{cmp::Reverse, collections::HashSet, path::PathBuf, process::ExitCode};

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use pybevy_lint::{
    comparison::simplify_type_for_display,
    output::{DiagnosticSeverity, format_diagnostic},
};

#[derive(Parser, Debug)]
#[command(name = "pybevy-lint")]
#[command(about = "Validates PyBevy Rust bindings against Python type stubs and Bevy API")]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Path to Rust source directory
    #[arg(long, default_value = ".")]
    rust_path: PathBuf,

    /// Path to Python stub directory
    #[arg(long, default_value = "pybevy")]
    python_path: PathBuf,

    /// Treat warnings as errors
    #[arg(long, global = true)]
    deny_warnings: bool,

    /// Only show errors (hide warnings)
    #[arg(long)]
    errors_only: bool,

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Only parse and display extracted classes (no validation)
    #[arg(long)]
    parse_only: bool,

    /// Path to configuration file
    #[arg(long)]
    config: Option<PathBuf>,

    /// Disable caching of Bevy API parsing results
    #[arg(long)]
    no_cache: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Compare PyBevy API against Bevy's public API
    #[command(alias = "compare-bevy")]
    Compare {
        /// Modules to compare (e.g., "input", "ecs transform"). Omit for all modules.
        #[arg(value_name = "MODULES")]
        modules: Vec<String>,

        /// Path to Bevy source directory
        #[arg(long)]
        bevy_path: Option<PathBuf>,

        /// Show only missing types (hide implemented)
        #[arg(long)]
        missing_only: bool,

        /// Show detailed method coverage for each type (default: true)
        #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
        show_methods: bool,

        /// Output format: text, markdown, json
        #[arg(long, default_value = "text")]
        format: String,

        /// Check usage of unimplemented types in Bevy examples (default: true)
        #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
        check_usage: bool,

        /// Show verbose output (intentional omissions, etc.)
        #[arg(long, short = 'v')]
        verbose: bool,

        /// Fail when an implemented Bevy enum has missing, extra, or mismatched variants
        #[arg(long)]
        check_enums: bool,

        /// Fail when an implemented method's signature differs from Bevy
        #[arg(long)]
        check_signatures: bool,
    },

    /// List available modules for comparison
    #[command(alias = "list-bevy-crates")]
    List {
        /// Path to Bevy source directory
        #[arg(long)]
        bevy_path: Option<PathBuf>,
    },

    /// Check usage of types in Bevy examples
    Usage {
        /// Path to Bevy source directory
        #[arg(long)]
        bevy_path: Option<PathBuf>,

        /// Types to check (comma-separated or multiple --type flags)
        #[arg(long, short = 't', value_delimiter = ',')]
        types: Vec<String>,

        /// Minimum usage count to display
        #[arg(long, default_value = "1")]
        min_usage: usize,
    },

    /// Validate PyBevy Rust bindings against Python type stubs
    Validate {
        /// Types to validate (e.g., "Transform", "PointLight"). Omit for all.
        #[arg(value_name = "TYPES")]
        types: Vec<String>,

        /// Filter diagnostics by code (e.g., "W006", "E003"). Can be specified multiple times.
        #[arg(long, short = 'c', value_delimiter = ',')]
        code: Vec<String>,

        /// Ignore diagnostics by code (e.g., "W006", "E003"). Can be specified multiple times.
        #[arg(long, short = 'i', value_delimiter = ',')]
        ignore: Vec<String>,
    },

    /// Show coverage summary table
    Coverage {
        /// Modules to check (omit for all)
        #[arg(value_name = "MODULES")]
        modules: Vec<String>,

        /// Path to Bevy source directory
        #[arg(long)]
        bevy_path: Option<PathBuf>,

        /// Sort by: name, coverage, types, methods
        #[arg(long, default_value = "coverage")]
        sort: String,

        /// Show only modules above this coverage threshold
        #[arg(long)]
        min_coverage: Option<f64>,

        /// Show only modules below this coverage threshold
        #[arg(long)]
        max_coverage: Option<f64>,
    },

    /// Report static exercise coverage of the public stub API
    TestCoverage {
        /// Types to check (e.g., "Transform", "PointLight"). Omit for all.
        #[arg(value_name = "TYPES")]
        types: Vec<String>,

        /// Path to test directory
        #[arg(long, default_value = "tests")]
        test_path: PathBuf,

        /// Output mode: "summary" or "diagnostics"
        #[arg(long, default_value = "summary")]
        output: String,

        /// Sort by: "name", "coverage", or "untested"
        #[arg(long, default_value = "coverage")]
        sort: String,

        /// Filter: only show classes below this coverage threshold
        #[arg(long)]
        max_coverage: Option<f64>,

        /// Show untested members per class
        #[arg(long)]
        show_members: bool,

        /// Check the committed monotonic exercise baseline
        #[arg(long, conflicts_with = "update_baseline")]
        check_baseline: bool,

        /// Refresh the reviewed exercise/debt baseline
        #[arg(long, conflicts_with = "check_baseline")]
        update_baseline: bool,

        /// Path to the exercise/debt baseline
        #[arg(long, default_value = "tests/api_exercise/baseline.json")]
        baseline_path: PathBuf,

        /// Path to exact, reasoned exercise exceptions
        #[arg(long, default_value = "tests/api_exercise/exceptions.json")]
        exceptions_path: PathBuf,

        /// Coverage.py JSON containing runtime line execution evidence
        #[arg(long)]
        execution_report: Option<PathBuf>,

        /// Group reviewed debt by module, operation kind, and risk
        #[arg(long)]
        debt_summary: bool,
    },

    /// Audit excluded types - check if any are used in Bevy examples
    Audit {
        /// Path to Bevy source directory
        #[arg(long)]
        bevy_path: Option<PathBuf>,

        /// Minimum usage count to flag as "should reconsider"
        #[arg(long, default_value = "1")]
        min_usage: usize,

        /// Also check excluded methods
        #[arg(long)]
        include_methods: bool,
    },

    /// Emit the fail-closed component field mapping artifact.
    EmitMappings {
        /// Path to the Bevy source checkout.
        #[arg(long)]
        bevy_path: Option<PathBuf>,

        /// Emit JSON to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,

        /// Write the artifact to a file instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Check or update the pinned Bevy 0.19 public-API inventories.
    BevyAudit {
        /// Rewrite the reviewed inventories and generated coverage block.
        #[arg(long)]
        update: bool,

        /// Reuse an exact pinned Bevy checkout instead of the managed cache.
        #[arg(long)]
        bevy_path: Option<PathBuf>,

        /// Directory used for the managed immutable Bevy checkout.
        #[arg(long, default_value = "target/bevy-api-audit")]
        cache_path: PathBuf,

        /// Raw upstream inventory output.
        #[arg(long, default_value = pybevy_lint::bevy_audit::DEFAULT_RAW_PATH)]
        raw_path: PathBuf,

        /// Reviewed binding-worthiness inventory output.
        #[arg(long, default_value = pybevy_lint::bevy_audit::DEFAULT_CLASSIFIED_PATH)]
        classified_path: PathBuf,

        /// Coverage document containing the generated block.
        #[arg(long, default_value = pybevy_lint::bevy_audit::DEFAULT_COVERAGE_PATH)]
        coverage_path: PathBuf,

        /// Reflection-oracle field mapping artifact.
        #[arg(long, default_value = "tests/parity/field_map.json")]
        field_map_path: PathBuf,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(has_errors) => {
            if has_errors {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("{}: {}", "error".red().bold(), e);
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<bool> {
    let args = Args::parse();

    let config = load_config(&args)?;

    match args.command {
        Some(Command::Compare {
            ref modules,
            ref bevy_path,
            missing_only,
            show_methods,
            ref format,
            check_usage,
            verbose,
            check_enums,
            check_signatures,
        }) => run_compare(
            &args,
            &config,
            bevy_path.clone(),
            modules.clone(),
            missing_only,
            show_methods,
            format,
            check_usage,
            verbose,
            check_enums,
            check_signatures,
        ),

        Some(Command::List { ref bevy_path }) => run_list(&args, config, bevy_path.clone()),

        Some(Command::Usage {
            ref bevy_path,
            ref types,
            min_usage,
        }) => run_usage(&args, config, bevy_path.clone(), types.clone(), min_usage),

        Some(Command::Validate {
            ref types,
            ref code,
            ref ignore,
        }) => run_validate(&args, &config, types, code, ignore),

        Some(Command::Coverage {
            ref modules,
            ref bevy_path,
            ref sort,
            min_coverage,
            max_coverage,
        }) => run_coverage(
            &args,
            &config,
            bevy_path.clone(),
            modules.clone(),
            sort,
            min_coverage,
            max_coverage,
        ),

        Some(Command::TestCoverage {
            ref types,
            ref test_path,
            ref output,
            ref sort,
            max_coverage,
            show_members,
            check_baseline,
            update_baseline,
            ref baseline_path,
            ref exceptions_path,
            ref execution_report,
            debt_summary,
        }) => run_test_coverage(
            &args,
            &config,
            types,
            test_path,
            output,
            sort,
            max_coverage,
            show_members,
            check_baseline,
            update_baseline,
            baseline_path,
            exceptions_path,
            execution_report.as_deref(),
            debt_summary,
        ),

        Some(Command::Audit {
            ref bevy_path,
            min_usage,
            include_methods,
        }) => run_audit(
            &args,
            &config,
            bevy_path.clone(),
            min_usage,
            include_methods,
        ),

        Some(Command::EmitMappings {
            ref bevy_path,
            json,
            ref output,
        }) => run_emit_mappings(&args, &config, bevy_path.clone(), json, output.as_deref()),

        Some(Command::BevyAudit {
            update,
            ref bevy_path,
            ref cache_path,
            ref raw_path,
            ref classified_path,
            ref coverage_path,
            ref field_map_path,
        }) => run_bevy_audit(
            &args,
            &config,
            update,
            bevy_path.as_deref(),
            cache_path,
            raw_path,
            classified_path,
            coverage_path,
            field_map_path,
        ),

        None => {
            use clap::CommandFactory;
            Args::command().print_help()?;
            println!();
            Ok(false)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_bevy_audit(
    args: &Args,
    config: &pybevy_lint::Config,
    update: bool,
    bevy_path: Option<&std::path::Path>,
    cache_path: &std::path::Path,
    raw_path: &std::path::Path,
    classified_path: &std::path::Path,
    coverage_path: &std::path::Path,
    field_map_path: &std::path::Path,
) -> Result<bool> {
    pybevy_lint::bevy_audit::check_cargo_public_api_version()?;
    let managed_checkout;
    let bevy_path = if let Some(path) = bevy_path {
        let revision = pybevy_lint::bevy_parser::cache::get_bevy_git_ref(path)?;
        if revision != pybevy_lint::bevy_audit::BEVY_REVISION {
            anyhow::bail!(
                "--bevy-path must be the pinned revision {}, found {}",
                pybevy_lint::bevy_audit::BEVY_REVISION,
                revision
            );
        }
        pybevy_lint::bevy_audit::install_pinned_lockfile(path)?;
        path
    } else {
        managed_checkout = pybevy_lint::bevy_audit::prepare_pinned_bevy(cache_path)?;
        &managed_checkout
    };

    let crates_to_parse: std::collections::BTreeSet<String> =
        config.bevy.crate_mappings.values().cloned().collect();
    let crate_refs: Vec<&str> = crates_to_parse.iter().map(String::as_str).collect();
    // The general-purpose developer cache predates tool/dependency provenance.
    // A release audit always extracts afresh from the pinned checkout and lockfile.
    let bevy_crates = pybevy_lint::parse_bevy_crates_with_cache(bevy_path, &crate_refs, false)?;
    pybevy_lint::bevy_audit::check_pinned_lockfile(bevy_path)?;
    let missing: Vec<_> = crate_refs
        .iter()
        .filter(|crate_name| !bevy_crates.contains_key(**crate_name))
        .copied()
        .collect();
    if !missing.is_empty() {
        anyhow::bail!(
            "pinned audit could not parse required Bevy crates: {}",
            missing.join(", ")
        );
    }

    let pybevy_classes = pybevy_lint::parse_rust_files(&args.rust_path)?;
    let comparison = pybevy_lint::compare_with_bevy(&pybevy_classes, &bevy_crates, config);
    let coverage_document = std::fs::read_to_string(coverage_path)?;
    let generated = pybevy_lint::bevy_audit::generate(
        &bevy_crates,
        &config.bevy,
        &comparison.report,
        &coverage_document,
    )?;
    pybevy_lint::bevy_audit::check_or_write(raw_path, &generated.raw_json, update)?;
    pybevy_lint::bevy_audit::check_or_write(classified_path, &generated.classified_json, update)?;
    pybevy_lint::bevy_audit::check_or_write(coverage_path, &generated.coverage_markdown, update)?;
    let field_map = pybevy_lint::mappings::generate_field_mappings_with_provenance(
        &pybevy_classes,
        &bevy_crates,
        &config.bevy,
        pybevy_lint::mappings::FieldMapProvenance::pinned_bevy_019(),
    )?;
    let serialized_field_map = serde_json::to_string_pretty(&field_map)? + "\n";
    pybevy_lint::bevy_audit::check_or_write(field_map_path, &serialized_field_map, update)?;
    eprintln!(
        "{} pinned Bevy 0.19 API audit artifacts {}",
        "success:".green().bold(),
        if update { "updated" } else { "are current" }
    );
    Ok(false)
}

fn load_config(args: &Args) -> Result<pybevy_lint::Config> {
    if let Some(ref config_path) = args.config {
        pybevy_lint::Config::load(config_path)
    } else {
        // Try to find config in standard locations
        let cwd = std::env::current_dir()?;
        match pybevy_lint::Config::find_and_load(&cwd)? {
            Some(config) => Ok(config),
            None => Ok(pybevy_lint::Config::default()),
        }
    }
}

fn run_emit_mappings(
    args: &Args,
    config: &pybevy_lint::Config,
    bevy_path: Option<PathBuf>,
    json: bool,
    output: Option<&std::path::Path>,
) -> Result<bool> {
    if !json {
        anyhow::bail!("emit-mappings currently requires --json");
    }
    let bevy_path = bevy_path
        .or_else(|| config.bevy.bevy_path())
        .ok_or_else(|| {
            anyhow::anyhow!("Bevy path not specified. Use --bevy-path or set bevy.path in config")
        })?;
    let pybevy_classes = pybevy_lint::parse_rust_files(&args.rust_path)?;
    let crates_to_parse: std::collections::BTreeSet<String> =
        config.bevy.crate_mappings.values().cloned().collect();
    let crate_refs: Vec<&str> = crates_to_parse.iter().map(String::as_str).collect();
    let bevy_crates =
        pybevy_lint::parse_bevy_crates_with_cache(&bevy_path, &crate_refs, !args.no_cache)?;
    let missing_crates: Vec<&String> = crates_to_parse
        .iter()
        .filter(|crate_name| !bevy_crates.contains_key(*crate_name))
        .collect();
    if !missing_crates.is_empty() {
        anyhow::bail!(
            "cannot emit a fail-closed field map; failed to parse Bevy crates: {}",
            missing_crates
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let artifact = pybevy_lint::mappings::generate_field_mappings(
        &pybevy_classes,
        &bevy_crates,
        &config.bevy,
    )?;
    let serialized = serde_json::to_string_pretty(&artifact)? + "\n";
    if let Some(output) = output {
        std::fs::write(output, serialized)?;
    } else {
        print!("{serialized}");
    }
    Ok(false)
}

fn run_validate(
    args: &Args,
    config: &pybevy_lint::Config,
    type_filter: &[String],
    code_filter: &[String],
    ignore_filter: &[String],
) -> Result<bool> {
    if args.verbose {
        eprintln!(
            "{} Parsing Rust files from {}...",
            "info:".blue().bold(),
            args.rust_path.display()
        );
    }

    let mut rust_classes = pybevy_lint::parse_rust_files(&args.rust_path)?;

    if !type_filter.is_empty() {
        let original_count = rust_classes.len();
        rust_classes.retain(|c| {
            type_filter.iter().any(|filter| {
                c.python_name.eq_ignore_ascii_case(filter)
                    || c.rust_name.eq_ignore_ascii_case(filter)
            })
        });
        if args.verbose {
            eprintln!(
                "{} Filtered to {} classes matching {:?}",
                "info:".blue().bold(),
                rust_classes.len(),
                type_filter
            );
        }
        if rust_classes.is_empty() {
            eprintln!(
                "{} No classes matched the filter {:?} (checked {} classes)",
                "warning:".yellow().bold(),
                type_filter,
                original_count
            );
            return Ok(false);
        }
    }

    if args.verbose {
        eprintln!(
            "{} Found {} PyClass definitions in Rust",
            "info:".blue().bold(),
            rust_classes.len()
        );
    }

    if args.parse_only {
        println!("\n{}", "=== Rust PyClass Definitions ===".green().bold());
        for class in &rust_classes {
            println!("\n{}", format_class_summary(class));
        }
        return Ok(false);
    }

    if args.verbose {
        eprintln!(
            "{} Parsing Python stubs from {}...",
            "info:".blue().bold(),
            args.python_path.display()
        );
    }

    let mut python_classes = pybevy_lint::parse_python_stubs(&args.python_path)?;

    // Filter Python classes by type names if specified (same filter as Rust)
    if !type_filter.is_empty() {
        python_classes.retain(|c| {
            type_filter
                .iter()
                .any(|filter| c.python_name.eq_ignore_ascii_case(filter))
        });
    }

    if args.verbose {
        eprintln!(
            "{} Found {} class definitions in Python stubs",
            "info:".blue().bold(),
            python_classes.len()
        );
    }

    let diagnostics = if type_filter.is_empty() {
        pybevy_lint::validate_with_config(&rust_classes, &python_classes, config)
    } else {
        pybevy_lint::validate_scoped_with_config(&rust_classes, &python_classes, config)
    };

    let mut error_count = 0;
    let mut warning_count = 0;

    let code_filter: Vec<String> = code_filter.iter().map(|c| c.to_uppercase()).collect();
    let ignore_filter: Vec<String> = ignore_filter.iter().map(|c| c.to_uppercase()).collect();
    let has_code_filter = !code_filter.is_empty();
    let has_ignore_filter = !ignore_filter.is_empty();

    for diagnostic in &diagnostics {
        let code_str = format!("{:?}", diagnostic.code);

        if has_ignore_filter && ignore_filter.iter().any(|f| code_str.contains(f)) {
            continue;
        }

        if has_code_filter && !code_filter.iter().any(|f| code_str.contains(f)) {
            continue;
        }

        match diagnostic.severity {
            DiagnosticSeverity::Error => {
                error_count += 1;
                println!("{}", format_diagnostic(diagnostic));
            }
            DiagnosticSeverity::Warning => {
                if !args.errors_only {
                    warning_count += 1;
                    println!("{}", format_diagnostic(diagnostic));
                }
            }
            DiagnosticSeverity::Info => {
                if args.verbose && !args.errors_only {
                    println!("{}", format_diagnostic(diagnostic));
                }
            }
        }
    }

    if error_count > 0 || warning_count > 0 {
        eprintln!();
        if error_count > 0 {
            eprintln!("{}: {} error(s) emitted", "error".red().bold(), error_count);
        }
        if warning_count > 0 {
            eprintln!(
                "{}: {} warning(s) emitted",
                "warning".yellow().bold(),
                warning_count
            );
        }
    } else {
        eprintln!("{} No issues found", "success:".green().bold());
    }

    let has_errors = error_count > 0 || (args.deny_warnings && warning_count > 0);
    Ok(has_errors)
}

fn run_compare(
    args: &Args,
    config: &pybevy_lint::Config,
    bevy_path: Option<PathBuf>,
    modules: Vec<String>,
    missing_only: bool,
    show_methods: bool,
    format: &str,
    check_usage: bool,
    verbose: bool,
    check_enums: bool,
    check_signatures: bool,
) -> Result<bool> {
    let bevy_path = bevy_path
        .or_else(|| config.bevy.bevy_path())
        .ok_or_else(|| {
            anyhow::anyhow!("Bevy path not specified. Use --bevy-path or set bevy.path in config")
        })?;

    if args.verbose {
        eprintln!(
            "{} Using Bevy source at {}",
            "info:".blue().bold(),
            bevy_path.display()
        );
    }

    if args.verbose {
        eprintln!("{} Parsing PyBevy Rust files...", "info:".blue().bold());
    }

    let pybevy_classes = pybevy_lint::parse_rust_files(&args.rust_path)?;

    if args.verbose {
        eprintln!(
            "{} Found {} PyClass definitions",
            "info:".blue().bold(),
            pybevy_classes.len()
        );
    }

    let crates_to_parse: Vec<String> = if modules.is_empty() {
        config.bevy.crate_mappings.values().cloned().collect()
    } else {
        modules
            .iter()
            .filter_map(|module| {
                if module.starts_with("bevy_") {
                    Some(module.clone())
                } else {
                    config.bevy.crate_mappings.get(module).cloned().or_else(|| {
                        let crate_name = format!("bevy_{}", module);
                        if config
                            .bevy
                            .crate_mappings
                            .values()
                            .any(|v| v == &crate_name)
                        {
                            Some(crate_name)
                        } else if config.bevy.is_crate_excluded(&crate_name) {
                            eprintln!(
                                "{} Module '{}' (crate '{}') is excluded in config.",
                                "warning:".yellow().bold(),
                                module,
                                crate_name
                            );
                            None
                        } else {
                            eprintln!(
                                "{} Unknown module '{}'. Use 'list' to see available modules.",
                                "warning:".yellow().bold(),
                                module
                            );
                            None
                        }
                    })
                }
            })
            .collect()
    };

    let crates_to_parse_refs: Vec<&str> = crates_to_parse.iter().map(|s| s.as_str()).collect();

    if args.verbose {
        eprintln!(
            "{} Parsing {} Bevy crates...",
            "info:".blue().bold(),
            crates_to_parse.len()
        );
    }

    let mut bevy_crates = pybevy_lint::parse_bevy_crates_with_cache(
        &bevy_path,
        &crates_to_parse_refs,
        !args.no_cache,
    )?;
    pybevy_lint::merge_reexported_bevy_types(
        &mut bevy_crates,
        &bevy_path,
        &config.bevy,
        &pybevy_classes,
        !args.no_cache,
    );

    if check_enums || check_signatures {
        let missing_crates: Vec<_> = crates_to_parse
            .iter()
            .filter(|crate_name| !bevy_crates.contains_key(*crate_name))
            .collect();
        if !missing_crates.is_empty() {
            anyhow::bail!(
                "contract check could not parse required Bevy crates: {}",
                missing_crates
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    if args.verbose {
        eprintln!(
            "{} Parsed {} Bevy crates",
            "info:".blue().bold(),
            bevy_crates.len()
        );
    }

    let result = pybevy_lint::compare_with_bevy(&pybevy_classes, &bevy_crates, config);

    let (usage_map, wrongly_excluded) = if check_usage {
        let examples_dir = bevy_path.join("examples");
        if examples_dir.exists() {
            let unimplemented: Vec<String> = result
                .report
                .crates
                .values()
                .flat_map(|c| c.types.iter())
                .filter(|t| !t.is_implemented)
                .map(|t| t.bevy_name.clone())
                .collect();

            if args.verbose {
                eprintln!(
                    "{} Checking {} unimplemented types in Bevy examples...",
                    "info:".blue().bold(),
                    unimplemented.len()
                );
            }

            let usage = check_type_usage_in_examples(&examples_dir, &unimplemented)?;

            // Filter out audit_ignore types (intentionally excluded despite example usage)
            let audit_ignore: HashSet<String> = config
                .bevy
                .excluded_types
                .audit_ignore
                .iter()
                .cloned()
                .collect();
            let excluded_types: Vec<String> =
                if crates_to_parse.len() < config.bevy.crate_mappings.len() {
                    // Filtering specific modules - only check global exclusions and module-prefixed ones
                    // that match the modules being compared
                    config
                        .bevy
                        .excluded_types
                        .types
                        .iter()
                        .filter(|t| {
                            if audit_ignore.contains(t.as_str()) {
                                return false;
                            }
                            // Keep global exclusions (no :: prefix)
                            if !t.contains("::") {
                                return true;
                            }
                            for crate_name in &crates_to_parse {
                                let short = crate_name.strip_prefix("bevy_").unwrap_or(crate_name);
                                if t.starts_with(&format!("{}::", crate_name))
                                    || t.starts_with(&format!("{}::", short))
                                {
                                    return true;
                                }
                            }
                            false
                        })
                        .cloned()
                        .collect()
                } else {
                    config
                        .bevy
                        .excluded_types
                        .types
                        .iter()
                        .filter(|t| !audit_ignore.contains(t.as_str()))
                        .cloned()
                        .collect()
                };

            let excluded_usage = if !excluded_types.is_empty() {
                if args.verbose {
                    eprintln!(
                        "{} Checking {} excluded types in Bevy examples...",
                        "info:".blue().bold(),
                        excluded_types.len()
                    );
                }
                let type_names: Vec<String> = excluded_types
                    .iter()
                    .map(|t| {
                        if let Some(pos) = t.rfind("::") {
                            t[pos + 2..].to_string()
                        } else {
                            t.clone()
                        }
                    })
                    .collect();
                let excluded_counts = check_type_usage_in_examples(&examples_dir, &type_names)?;
                let mut found: Vec<_> = excluded_counts
                    .into_iter()
                    .filter(|(_, count)| *count > 0)
                    .collect();
                found.sort_by_key(|entry| Reverse(entry.1));
                found
            } else {
                Vec::new()
            };

            (Some(usage), excluded_usage)
        } else {
            eprintln!(
                "{} Examples directory not found: {}",
                "warning:".yellow().bold(),
                examples_dir.display()
            );
            (None, Vec::new())
        }
    } else {
        (None, Vec::new())
    };

    match format {
        "markdown" | "md" => print_coverage_markdown(&result.report, missing_only, show_methods),
        "json" => print_coverage_json(&result.report),
        _ => print_coverage_text_with_usage(
            &result.report,
            missing_only,
            show_methods,
            usage_map.as_ref(),
            &wrongly_excluded,
            config,
            verbose,
        ),
    }

    let enum_mismatch = result.report.enum_representation_mismatches > 0
        || result.report.missing_variants > 0
        || result.report.mismatched_variants > 0
        || result.report.extra_variants > 0;
    Ok((check_enums && enum_mismatch)
        || (check_signatures && result.report.signature_mismatches > 0))
}

fn run_list(_args: &Args, config: pybevy_lint::Config, bevy_path: Option<PathBuf>) -> Result<bool> {
    let bevy_path = bevy_path
        .or_else(|| config.bevy.bevy_path())
        .ok_or_else(|| {
            anyhow::anyhow!("Bevy path not specified. Use --bevy-path or set bevy.path in config")
        })?;

    let crates_dir = bevy_path.join("crates");
    if !crates_dir.exists() {
        anyhow::bail!("Bevy crates directory not found: {}", crates_dir.display());
    }

    println!("{}", "Available Bevy crates:".green().bold());
    println!();

    let mut crates: Vec<String> = std::fs::read_dir(&crates_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.starts_with("bevy_"))
        .collect();

    crates.sort();

    let excluded = config.bevy.excluded_crates_set();
    let mapped: std::collections::HashSet<&str> = config
        .bevy
        .crate_mappings
        .values()
        .map(|s| s.as_str())
        .collect();

    for crate_name in &crates {
        let is_excluded = excluded.contains(crate_name.as_str());
        let is_mapped = mapped.contains(crate_name.as_str());

        let status = if is_excluded {
            "(excluded)".dimmed()
        } else if is_mapped {
            "(mapped)".green()
        } else {
            "(not mapped)".yellow()
        };

        println!("  {} {}", crate_name, status);
    }

    println!();
    println!(
        "Total: {} crates ({} mapped, {} excluded)",
        crates.len(),
        mapped.len(),
        excluded.len()
    );

    Ok(false)
}

/// Check usage of types in Bevy examples directory
fn check_type_usage_in_examples(
    examples_dir: &std::path::Path,
    types: &[String],
) -> Result<std::collections::HashMap<String, usize>> {
    use std::collections::{HashMap, HashSet};

    let type_set: HashSet<&str> = types.iter().map(|t| t.as_str()).collect();
    let mut usage_counts: HashMap<String, usize> = types.iter().map(|t| (t.clone(), 0)).collect();

    let mut parser = tree_sitter::Parser::new();
    let rust_lang = tree_sitter_rust::LANGUAGE;
    parser
        .set_language(&rust_lang.into())
        .expect("Failed to set Rust language for tree-sitter");

    fn visit_dir(
        dir: &std::path::Path,
        type_set: &HashSet<&str>,
        counts: &mut HashMap<String, usize>,
        parser: &mut tree_sitter::Parser,
    ) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                visit_dir(&path, type_set, counts, parser)?;
            } else if path.extension().map(|e| e == "rs").unwrap_or(false)
                && let Ok(content) = std::fs::read_to_string(&path)
            {
                count_type_usage_tree_sitter(&content, type_set, counts, parser);
            }
        }
        Ok(())
    }

    visit_dir(examples_dir, &type_set, &mut usage_counts, &mut parser)?;
    Ok(usage_counts)
}

/// Count type usages in a Rust file using tree-sitter AST parsing.
///
/// Counts `type_identifier` nodes, identifiers in `use_declaration` subtrees, and
/// PascalCase `scoped_identifier` segments (e.g., `TypeName::method()`); skips
/// comments, string literals, and method-call children.
fn count_type_usage_tree_sitter(
    source: &str,
    type_set: &std::collections::HashSet<&str>,
    counts: &mut std::collections::HashMap<String, usize>,
    parser: &mut tree_sitter::Parser,
) {
    let Some(tree) = parser.parse(source, None) else {
        return;
    };

    let source_bytes = source.as_bytes();

    fn walk_node(
        node: tree_sitter::Node,
        source: &[u8],
        type_set: &std::collections::HashSet<&str>,
        counts: &mut std::collections::HashMap<String, usize>,
        in_use_decl: bool,
    ) {
        let kind = node.kind();

        match kind {
            "line_comment" | "block_comment" | "string_literal" | "raw_string_literal" => return,
            _ => {}
        }

        let is_use_subtree = in_use_decl || kind == "use_declaration";

        // type_identifier: always a type reference (annotations, generics, bounds)
        if kind == "type_identifier" {
            if let Ok(text) = node.utf8_text(source)
                && type_set.contains(text)
            {
                *counts.entry(text.to_string()).or_insert(0) += 1;
            }
        }
        // identifier inside use declarations (imports)
        else if kind == "identifier" && is_use_subtree {
            if let Ok(text) = node.utf8_text(source)
                && text.starts_with(|c: char| c.is_uppercase())
                && type_set.contains(text)
            {
                *counts.entry(text.to_string()).or_insert(0) += 1;
            }
        }
        // scoped_identifier: path like `TypeName::method`, check the leftmost PascalCase segment
        else if kind == "scoped_identifier" && !is_use_subtree {
            if let Some(path_node) = node.child_by_field_name("path")
                && let Ok(text) = path_node.utf8_text(source)
            {
                // Only count if it's a simple PascalCase identifier (not a nested path)
                if !text.contains(':')
                    && text.starts_with(|c: char| c.is_uppercase())
                    && type_set.contains(text)
                {
                    // Skip if parent is field_expression (method call like `obj.Type::method()`)
                    let parent_kind = node.parent().map(|p| p.kind()).unwrap_or("");
                    if parent_kind != "field_expression" {
                        *counts.entry(text.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk_node(child, source, type_set, counts, is_use_subtree);
        }
    }

    walk_node(tree.root_node(), source_bytes, type_set, counts, false);
}

fn run_usage(
    args: &Args,
    config: pybevy_lint::Config,
    bevy_path: Option<PathBuf>,
    types: Vec<String>,
    min_usage: usize,
) -> Result<bool> {
    let bevy_path = bevy_path
        .or_else(|| config.bevy.bevy_path())
        .ok_or_else(|| {
            anyhow::anyhow!("Bevy path not specified. Use --bevy-path or set bevy.path in config")
        })?;

    let examples_dir = bevy_path.join("examples");
    if !examples_dir.exists() {
        anyhow::bail!("Examples directory not found: {}", examples_dir.display());
    }

    if types.is_empty() {
        anyhow::bail!("No types specified. Use --type TYPE1,TYPE2 or -t TYPE");
    }

    if args.verbose {
        eprintln!(
            "{} Checking {} types in {}...",
            "info:".blue().bold(),
            types.len(),
            examples_dir.display()
        );
    }

    let usage = check_type_usage_in_examples(&examples_dir, &types)?;

    let mut sorted: Vec<_> = usage.into_iter().collect();
    sorted.sort_by_key(|entry| Reverse(entry.1));

    println!();
    println!("{}", "Type Usage in Bevy Examples".bold().cyan());
    println!("{}", "═══════════════════════════════════════".bold());
    println!();

    let mut shown = 0;
    for (type_name, count) in &sorted {
        if *count >= min_usage {
            let count_display = if *count > 0 {
                format!("{}", count).green()
            } else {
                format!("{}", count).dimmed()
            };
            println!("  {:>4}  {}", count_display, type_name);
            shown += 1;
        }
    }

    println!();
    println!(
        "Shown {}/{} types (min usage: {})",
        shown,
        sorted.len(),
        min_usage
    );

    Ok(false)
}

fn run_coverage(
    args: &Args,
    config: &pybevy_lint::Config,
    bevy_path: Option<PathBuf>,
    modules: Vec<String>,
    sort: &str,
    min_coverage: Option<f64>,
    max_coverage: Option<f64>,
) -> Result<bool> {
    let bevy_path = bevy_path
        .or_else(|| config.bevy.bevy_path())
        .ok_or_else(|| {
            anyhow::anyhow!("Bevy path not specified. Use --bevy-path or set bevy.path in config")
        })?;

    if args.verbose {
        eprintln!(
            "{} Using Bevy source at {}",
            "info:".blue().bold(),
            bevy_path.display()
        );
    }

    let pybevy_classes = pybevy_lint::parse_rust_files(&args.rust_path)?;

    let audit_config = modules.is_empty();
    let crates_to_parse: Vec<String> = if modules.is_empty() {
        config.bevy.crate_mappings.values().cloned().collect()
    } else {
        modules
            .iter()
            .filter_map(|module| {
                if module.starts_with("bevy_") {
                    Some(module.clone())
                } else {
                    config.bevy.crate_mappings.get(module).cloned().or_else(|| {
                        let crate_name = format!("bevy_{}", module);
                        if config
                            .bevy
                            .crate_mappings
                            .values()
                            .any(|v| v == &crate_name)
                        {
                            Some(crate_name)
                        } else {
                            eprintln!(
                                "{} Unknown module '{}'. Use 'list' to see available modules.",
                                "warning:".yellow().bold(),
                                module
                            );
                            None
                        }
                    })
                }
            })
            .collect()
    };

    let crates_to_parse_refs: Vec<&str> = crates_to_parse.iter().map(|s| s.as_str()).collect();
    let mut bevy_crates = pybevy_lint::parse_bevy_crates_with_cache(
        &bevy_path,
        &crates_to_parse_refs,
        !args.no_cache,
    )?;
    pybevy_lint::merge_reexported_bevy_types(
        &mut bevy_crates,
        &bevy_path,
        &config.bevy,
        &pybevy_classes,
        !args.no_cache,
    );

    let result = pybevy_lint::compare_with_bevy(&pybevy_classes, &bevy_crates, config);

    print_coverage_table(&result.report, sort, min_coverage, max_coverage);

    // A filtered run parses a subset of crates, which would make every entry for
    // an unparsed crate look stale. Only a full run can judge the config.
    if !audit_config {
        return Ok(false);
    }
    let source = pybevy_lint::config_audit::BevySource::scan(&bevy_path);
    if source.is_empty() {
        eprintln!(
            "{} no Bevy crates found under {}; skipping the configuration audit",
            "warning:".yellow().bold(),
            bevy_path.join("crates").display()
        );
        return Ok(false);
    }
    let diagnostics = pybevy_lint::config_audit::audit(config, &source, &pybevy_classes);
    report_config_audit(args, &diagnostics)
}

/// Print configuration findings and report whether the run should fail.
fn report_config_audit(
    args: &Args,
    diagnostics: &[pybevy_lint::output::Diagnostic],
) -> Result<bool> {
    let mut errors = 0;
    let mut warnings = 0;
    for diagnostic in diagnostics {
        match diagnostic.severity {
            DiagnosticSeverity::Error => {
                errors += 1;
                println!("{}", format_diagnostic(diagnostic));
            }
            DiagnosticSeverity::Warning if !args.errors_only => {
                warnings += 1;
                println!("{}", format_diagnostic(diagnostic));
            }
            _ => {}
        }
    }
    if errors > 0 {
        eprintln!(
            "{}: {} stale configuration entr{}",
            "error".red().bold(),
            errors,
            if errors == 1 { "y" } else { "ies" }
        );
    }
    if warnings > 0 {
        eprintln!(
            "{}: {} excluded type(s) implemented in PyBevy",
            "warning".yellow().bold(),
            warnings
        );
    }
    Ok(errors > 0 || (args.deny_warnings && warnings > 0))
}

fn run_test_coverage(
    args: &Args,
    config: &pybevy_lint::Config,
    type_filter: &[String],
    test_path: &PathBuf,
    output_mode: &str,
    sort: &str,
    max_coverage: Option<f64>,
    show_members: bool,
    check_baseline_mode: bool,
    update_baseline_mode: bool,
    baseline_path: &PathBuf,
    exceptions_path: &PathBuf,
    execution_report: Option<&std::path::Path>,
    debt_summary: bool,
) -> Result<bool> {
    if (check_baseline_mode || update_baseline_mode) && !type_filter.is_empty() {
        anyhow::bail!("baseline check/update does not accept a type filter");
    }
    if args.verbose {
        eprintln!(
            "{} Parsing public stubs from {}...",
            "info:".blue().bold(),
            args.python_path.display()
        );
    }

    let stub_classes = pybevy_lint::parse_python_stubs(&args.python_path)?;
    let mut catalog = pybevy_lint::test_coverage::ApiCatalog::from_stub_classes(stub_classes);
    catalog.add_stub_symbols(pybevy_lint::parse_python_stub_symbols(&args.python_path)?);
    let prelude_path = args.python_path.join("prelude.py");
    if prelude_path.exists() {
        catalog.add_wildcard_reexports(pybevy_lint::parse_python_reexports(&prelude_path)?);
    }

    if !type_filter.is_empty() {
        catalog.retain(|path, class| {
            type_filter.iter().any(|filter| {
                class.python_name.eq_ignore_ascii_case(filter)
                    || path.as_str().eq_ignore_ascii_case(filter)
            })
        });
    }

    if args.verbose {
        eprintln!(
            "{} Found {} public stub classes",
            "info:".blue().bold(),
            catalog.len()
        );
    }

    let effective_test_path = if test_path.to_str() == Some("tests") {
        PathBuf::from(&config.test_coverage.test_path)
    } else {
        test_path.clone()
    };

    if args.verbose {
        eprintln!(
            "{} Parsing test files from {}...",
            "info:".blue().bold(),
            effective_test_path.display()
        );
    }

    let mut test_usage =
        pybevy_lint::test_coverage::parse_test_files(&effective_test_path, &catalog)?;
    if let Some(path) = execution_report {
        let report = pybevy_lint::test_coverage::ExecutionReport::read(path)?;
        pybevy_lint::test_coverage::apply_execution_report(&mut test_usage, &report);
    }

    if args.verbose {
        eprintln!(
            "{} Found {} classes referenced in tests",
            "info:".blue().bold(),
            test_usage.classes.len()
        );
    }

    if update_baseline_mode {
        pybevy_lint::test_coverage::update_baseline(
            &catalog,
            &test_usage,
            baseline_path,
            exceptions_path,
        )?;
        println!("updated {}", baseline_path.display());
        return Ok(false);
    }
    if check_baseline_mode {
        let check = pybevy_lint::test_coverage::check_baseline(
            &catalog,
            &test_usage,
            baseline_path,
            exceptions_path,
        )?;
        if check.is_ok() {
            println!("API exercise baseline is current");
            return Ok(false);
        }
        for error in &check.errors {
            eprintln!("{} {error}", "error:".red().bold());
        }
        eprintln!(
            "{} {} baseline regression(s)",
            "error:".red().bold(),
            check.errors.len()
        );
        return Ok(true);
    }

    let report =
        pybevy_lint::test_coverage::analyze_coverage(&catalog, &test_usage, &config.test_coverage);

    match output_mode {
        "diagnostics" => {
            let diagnostics = pybevy_lint::test_coverage::generate_diagnostics(&report);
            let mut count = 0;
            for diag in &diagnostics {
                println!("{}", format_diagnostic(diag));
                count += 1;
            }
            if count > 0 {
                eprintln!(
                    "\n{}: {} info diagnostic(s) emitted",
                    "info".blue().bold(),
                    count
                );
            } else {
                eprintln!(
                    "{} All classes have test coverage",
                    "success:".green().bold()
                );
            }
        }
        _ => {
            // summary (default)
            let table = pybevy_lint::test_coverage::format_summary_table_detailed(
                &report,
                show_members,
                sort,
                max_coverage,
            );
            print!("{}", table);
        }
    }

    if debt_summary {
        print!(
            "{}",
            pybevy_lint::test_coverage::format_debt_summary(baseline_path)?
        );
    }

    Ok(false)
}

fn run_audit(
    args: &Args,
    config: &pybevy_lint::Config,
    bevy_path: Option<PathBuf>,
    min_usage: usize,
    include_methods: bool,
) -> Result<bool> {
    let bevy_path = bevy_path
        .or_else(|| config.bevy.bevy_path())
        .ok_or_else(|| {
            anyhow::anyhow!("Bevy path not specified. Use --bevy-path or set bevy.path in config")
        })?;

    let examples_dir = bevy_path.join("examples");
    if !examples_dir.exists() {
        anyhow::bail!("Examples directory not found: {}", examples_dir.display());
    }

    let audit_ignore: HashSet<String> = config
        .bevy
        .excluded_types
        .audit_ignore
        .iter()
        .cloned()
        .collect();
    let excluded_types: Vec<String> = config
        .bevy
        .excluded_types
        .types
        .iter()
        .filter(|t| !audit_ignore.contains(t.as_str()))
        .cloned()
        .collect();
    let excluded_patterns: Vec<String> = config.bevy.excluded_types.patterns.clone();

    if args.verbose {
        eprintln!(
            "{} Checking {} excluded types and {} patterns against Bevy examples...",
            "info:".blue().bold(),
            excluded_types.len(),
            excluded_patterns.len()
        );
    }

    println!();
    println!(
        "{}",
        "Audit: Excluded Types Usage in Bevy Examples".bold().cyan()
    );
    println!(
        "{}",
        "═══════════════════════════════════════════════════════════════════════════════".bold()
    );
    println!();

    if !excluded_types.is_empty() {
        let usage = check_type_usage_in_examples(&examples_dir, &excluded_types)?;

        let mut found: Vec<_> = usage
            .into_iter()
            .filter(|(_, count)| *count >= min_usage)
            .collect();
        found.sort_by_key(|entry| Reverse(entry.1));

        if found.is_empty() {
            println!("{}", "Excluded Types".bold());
            println!(
                "  {} All {} excluded types have zero usage in examples ✓",
                "OK:".green().bold(),
                excluded_types.len()
            );
        } else {
            println!(
                "{} {} excluded types found in examples:",
                "WARNING:".yellow().bold(),
                found.len()
            );
            println!();
            println!("  {:40} {:>10}", "Type".bold(), "Usage".bold());
            println!("  {:40} {:>10}", "─".repeat(40), "─".repeat(10));

            for (type_name, count) in &found {
                println!(
                    "  {:40} {:>10}",
                    type_name.yellow(),
                    count.to_string().yellow()
                );
            }
            println!();
            println!(
                "  {} Consider removing these from excluded_types in .pybevy-lint.toml",
                "Suggestion:".blue().bold()
            );
        }
        println!();
    }

    if !excluded_patterns.is_empty() {
        println!("{}", "Excluded Patterns".bold());
        println!("  Patterns: {}", excluded_patterns.join(", ").dimmed());
        println!(
            "  {} Pattern checking requires Bevy parsing - use 'compare' for full coverage",
            "Note:".blue().bold()
        );
        println!();
    }

    if include_methods {
        let excluded_method_patterns: Vec<String> = config.bevy.excluded_methods.patterns.clone();

        if !excluded_method_patterns.is_empty() {
            println!("{}", "Excluded Method Patterns".bold());
            println!("  Patterns: {}", excluded_method_patterns.len());

            // For methods, we check common method names that might be in examples
            let common_methods: Vec<String> = excluded_method_patterns
                .iter()
                .filter(|p| !p.contains('*')) // Only check exact patterns
                .cloned()
                .collect();

            if !common_methods.is_empty() {
                let usage = check_type_usage_in_examples(&examples_dir, &common_methods)?;
                let mut found: Vec<_> = usage
                    .into_iter()
                    .filter(|(_, count)| *count >= min_usage)
                    .collect();
                found.sort_by_key(|entry| Reverse(entry.1));

                if !found.is_empty() {
                    println!();
                    println!(
                        "  {} {} excluded methods found in examples:",
                        "WARNING:".yellow().bold(),
                        found.len()
                    );
                    for (method_name, count) in found.iter().take(10) {
                        println!("    {} ({})", method_name.yellow(), count);
                    }
                    if found.len() > 10 {
                        println!("    ... and {} more", found.len() - 10);
                    }
                }
            }
            println!();
        }
    }

    println!("{}", "Audit Summary".bold());
    println!("  Excluded types checked:    {}", excluded_types.len());
    if !audit_ignore.is_empty() {
        println!(
            "  Audit-ignored types:       {} (intentional exclusions, skipped)",
            audit_ignore.len()
        );
    }
    println!("  Excluded patterns:         {}", excluded_patterns.len());
    if include_methods {
        println!(
            "  Excluded method patterns:  {}",
            config.bevy.excluded_methods.patterns.len()
        );
    }
    println!();

    Ok(false)
}

fn print_coverage_table(
    report: &pybevy_lint::CoverageReport,
    sort: &str,
    min_coverage: Option<f64>,
    max_coverage: Option<f64>,
) {
    println!();
    println!("{}", "PyBevy API Coverage Summary".bold().cyan());
    println!("{}", "═══════════════════════════════════════════════════════════════════════════════════════════════".bold());
    println!();

    // Row: (name, type_coverage, matched_types, total_types, impl_methods, total_methods,
    //       method_coverage, sig_mismatches, missing_fields, missing_variants, extra_variants, extra_methods)
    #[allow(clippy::type_complexity)]
    let mut rows: Vec<(
        String,
        f64,
        usize,
        usize,
        usize,
        usize,
        f64,
        usize,
        usize,
        usize,
        usize,
        usize,
    )> = report
        .crates
        .iter()
        .filter(|(_, cov)| {
            let pct = cov.coverage_percent();
            min_coverage.is_none_or(|min| pct >= min) && max_coverage.is_none_or(|max| pct <= max)
        })
        .map(|(name, cov)| {
            // Get method coverage for this crate (only implemented types)
            let (
                impl_methods,
                total_methods,
                sig_mismatches,
                missing_fields,
                missing_variants,
                extra_variants,
                extra_methods,
            ) = cov.types.iter().filter(|t| t.is_implemented).fold(
                (0, 0, 0, 0, 0, 0, 0),
                |(m, t, sig, fields, vars, extra_vars, extra_meths), typ| {
                    let type_sig_mismatches = typ
                        .methods
                        .iter()
                        .filter(|method| method.is_implemented && !method.signature_matches)
                        .count();
                    let type_missing_fields = typ.bevy_field_count - typ.matched_field_count;
                    let type_missing_variants = typ.bevy_variant_count - typ.matched_variant_count;
                    (
                        m + typ.matched_method_count,
                        t + typ.bevy_method_count,
                        sig + type_sig_mismatches,
                        fields + type_missing_fields,
                        vars + type_missing_variants,
                        extra_vars + typ.extra_variant_count,
                        extra_meths + typ.extra_method_count,
                    )
                },
            );
            let method_pct = if total_methods > 0 {
                (impl_methods as f64 / total_methods as f64) * 100.0
            } else {
                100.0
            };
            (
                name.clone(),
                cov.coverage_percent(),
                cov.matched_count,
                cov.bevy_type_count,
                impl_methods,
                total_methods,
                method_pct,
                sig_mismatches,
                missing_fields,
                missing_variants,
                extra_variants,
                extra_methods,
            )
        })
        .collect();

    match sort {
        "name" => rows.sort_by(|a, b| a.0.cmp(&b.0)),
        "coverage" => rows.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        }),
        "types" => rows.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| a.0.cmp(&b.0))),
        "methods" => rows.sort_by(|a, b| b.5.cmp(&a.5).then_with(|| a.0.cmp(&b.0))),
        _ => rows.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        }),
    }

    println!(
        "{:20} {:>10} {:>10} {:>14} {:>10} {:>18}",
        "Module".bold(),
        "Types".bold(),
        "Type %".bold(),
        "Methods".bold(),
        "Meth %".bold(),
        "Issues".bold(),
    );
    println!(
        "{:20} {:>10} {:>10} {:>14} {:>10} {:>18}",
        "─".repeat(20),
        "─".repeat(10),
        "─".repeat(10),
        "─".repeat(14),
        "─".repeat(10),
        "─".repeat(18),
    );

    for (
        name,
        type_coverage,
        matched,
        total,
        methods_impl,
        methods_total,
        method_pct,
        sig_mismatches,
        missing_fields,
        missing_variants,
        extra_variants,
        extra_methods,
    ) in &rows
    {
        // Strip bevy_ prefix for cleaner display
        let display_name = name.strip_prefix("bevy_").unwrap_or(name);

        let type_coverage_str = format!("{:.1}%", type_coverage);
        let type_coverage_colored = if *type_coverage >= 80.0 {
            type_coverage_str.green()
        } else if *type_coverage >= 50.0 {
            type_coverage_str.yellow()
        } else {
            type_coverage_str.red()
        };

        let types_str = format!("{}/{}", matched, total);

        let (methods_str, method_pct_str) = if *methods_total > 0 {
            let pct_str = format!("{:.0}%", method_pct);
            let pct_colored = if *method_pct >= 80.0 {
                pct_str.green()
            } else if *method_pct >= 50.0 {
                pct_str.yellow()
            } else {
                pct_str.red()
            };
            (
                format!("{:>14}", format!("{}/{}", methods_impl, methods_total)),
                format!("{:>6}", pct_colored),
            )
        } else {
            (format!("{:>14}", "-"), format!("{:>6}", "-".dimmed()))
        };

        // Build issues string - compact format showing all issue types
        let has_issues = *sig_mismatches > 0
            || *missing_fields > 0
            || *missing_variants > 0
            || *extra_variants > 0
            || *extra_methods > 0;
        let issues_str = if has_issues {
            let mut parts = Vec::new();
            if *sig_mismatches > 0 {
                parts.push(format!("{}sig", sig_mismatches));
            }
            if *missing_fields > 0 {
                parts.push(format!("{}fld", missing_fields));
            }
            if *missing_variants > 0 {
                parts.push(format!("{}var", missing_variants));
            }
            if *extra_variants > 0 {
                parts.push(format!("+{}var", extra_variants));
            }
            if *extra_methods > 0 {
                parts.push(format!("+{}mth", extra_methods));
            }
            parts.join(" ").yellow().to_string()
        } else {
            "✓".green().to_string()
        };

        println!(
            "{:20} {:>10} {:>10} {} {} {}",
            display_name, types_str, type_coverage_colored, methods_str, method_pct_str, issues_str,
        );
    }

    println!(
        "{:20} {:>10} {:>10} {:>14} {:>10} {:>18}",
        "─".repeat(20),
        "─".repeat(10),
        "─".repeat(10),
        "─".repeat(14),
        "─".repeat(10),
        "─".repeat(18),
    );

    let total_type_pct = report.type_coverage_percent();
    let total_type_pct_str = format!("{:.1}%", total_type_pct);
    let total_type_colored = if total_type_pct >= 80.0 {
        total_type_pct_str.green().bold()
    } else if total_type_pct >= 50.0 {
        total_type_pct_str.yellow().bold()
    } else {
        total_type_pct_str.red().bold()
    };

    let method_pct = report.implemented_method_coverage_percent();
    let method_pct_str = format!("{:.0}%", method_pct);
    let method_pct_colored = if method_pct >= 80.0 {
        method_pct_str.green().bold()
    } else if method_pct >= 50.0 {
        method_pct_str.yellow().bold()
    } else {
        method_pct_str.red().bold()
    };

    let total_sig_mismatches: usize = rows.iter().map(|r| r.7).sum();
    let total_missing_fields: usize = rows.iter().map(|r| r.8).sum();
    let total_missing_variants: usize = rows.iter().map(|r| r.9).sum();
    let total_extra_variants: usize = rows.iter().map(|r| r.10).sum();
    let total_extra_methods: usize = rows.iter().map(|r| r.11).sum();
    let has_total_issues = total_sig_mismatches > 0
        || total_missing_fields > 0
        || total_missing_variants > 0
        || total_extra_variants > 0
        || total_extra_methods > 0;
    let total_issues_str = if has_total_issues {
        let mut parts = Vec::new();
        if total_sig_mismatches > 0 {
            parts.push(format!("{}sig", total_sig_mismatches));
        }
        if total_missing_fields > 0 {
            parts.push(format!("{}fld", total_missing_fields));
        }
        if total_missing_variants > 0 {
            parts.push(format!("{}var", total_missing_variants));
        }
        if total_extra_variants > 0 {
            parts.push(format!("+{}var", total_extra_variants));
        }
        if total_extra_methods > 0 {
            parts.push(format!("+{}mth", total_extra_methods));
        }
        parts.join(" ").yellow().bold().to_string()
    } else {
        "✓".green().bold().to_string()
    };

    println!(
        "{:20} {:>10} {:>10} {:>14} {:>6} {}",
        "TOTAL".bold(),
        format!("{}/{}", report.matched_types, report.total_bevy_types),
        total_type_colored,
        format!(
            "{}/{}",
            report.implemented_type_matched_methods, report.implemented_type_bevy_methods
        ),
        method_pct_colored,
        total_issues_str,
    );

    println!();
    println!("{}", "Summary".bold());
    println!("  Modules:  {} analyzed", rows.len());
    println!(
        "  Types:    {} implemented of {} total ({:.1}%)",
        report.matched_types, report.total_bevy_types, total_type_pct
    );
    println!(
        "  Methods:  {} implemented of {} in implemented types ({:.1}%)",
        report.implemented_type_matched_methods, report.implemented_type_bevy_methods, method_pct
    );
    if report.total_bevy_fields > 0 {
        let field_pct = report.field_coverage_percent();
        println!(
            "  Fields:   {} implemented of {} in implemented types ({:.1}%)",
            report.matched_fields, report.total_bevy_fields, field_pct
        );
    }
    if report.total_bevy_variants > 0 {
        let variant_pct = report.variant_coverage_percent();
        println!(
            "  Variants: {} implemented of {} in implemented enum types ({:.1}%)",
            report.matched_variants, report.total_bevy_variants, variant_pct
        );
        if report.extra_variants > 0 {
            println!(
                "            {} extra variants in PyBevy (not in Bevy)",
                report.extra_variants
            );
        }
        if report.missing_variants > 0 {
            println!(
                "            {} missing variants in PyBevy",
                report.missing_variants
            );
        }
        if report.mismatched_variants > 0 {
            println!(
                "            {} variants with mismatched payload shapes",
                report.mismatched_variants
            );
        }
    }
    if report.enum_representation_mismatches > 0 {
        println!(
            "  Enum repr: {} Bevy enums use non-enum PyBevy wrappers",
            report.enum_representation_mismatches
        );
    }
    if report.extra_methods > 0 {
        println!(
            "  Extra:    {} methods in PyBevy not in Bevy (may need review)",
            report.extra_methods
        );
    }
    println!();
}

fn print_coverage_text_with_usage(
    report: &pybevy_lint::CoverageReport,
    missing_only: bool,
    show_methods: bool,
    usage_map: Option<&std::collections::HashMap<String, usize>>,
    wrongly_excluded: &[(String, usize)],
    config: &pybevy_lint::Config,
    verbose: bool,
) {
    println!();
    println!(
        "{}",
        "═══════════════════════════════════════════════════════════".bold()
    );
    println!("{}", "           PyBevy API Coverage Report".bold().cyan());
    println!(
        "{}",
        "═══════════════════════════════════════════════════════════".bold()
    );
    println!();

    println!("{}", "Overall Coverage:".bold());
    println!(
        "  Types:   {}/{} ({:.1}%)",
        report.matched_types,
        report.total_bevy_types,
        report.type_coverage_percent()
    );
    println!(
        "  Methods: {}/{} ({:.1}%) overall, {}/{} ({:.1}%) in implemented types",
        report.matched_methods,
        report.total_bevy_methods,
        report.method_coverage_percent(),
        report.implemented_type_matched_methods,
        report.implemented_type_bevy_methods,
        report.implemented_method_coverage_percent()
    );
    if report.total_bevy_fields > 0 {
        println!(
            "  Fields:  {}/{} ({:.1}%) in implemented types",
            report.matched_fields,
            report.total_bevy_fields,
            report.field_coverage_percent()
        );
    }
    if report.total_bevy_variants > 0 {
        let variant_info = if report.extra_variants > 0
            || report.missing_variants > 0
            || report.mismatched_variants > 0
        {
            format!(
                " (+{} extra, -{} missing, ~{} mismatched)",
                report.extra_variants, report.missing_variants, report.mismatched_variants
            )
        } else {
            String::new()
        };
        println!(
            "  Variants: {}/{} ({:.1}%) in implemented enum types{}",
            report.matched_variants,
            report.total_bevy_variants,
            report.variant_coverage_percent(),
            variant_info
        );
    }
    if report.enum_representation_mismatches > 0 {
        println!(
            "  Enum representation mismatches: {}",
            report.enum_representation_mismatches
        );
    }
    println!();

    let mut crates: Vec<_> = report.crates.iter().collect();
    crates.sort_by(|a, b| a.0.cmp(b.0));

    for (crate_name, crate_coverage) in crates {
        let coverage_pct = crate_coverage.coverage_percent();

        let crate_display = if coverage_pct >= 80.0 {
            crate_name.green().bold()
        } else if coverage_pct >= 50.0 {
            crate_name.yellow().bold()
        } else {
            crate_name.red().bold()
        };

        println!(
            "{} {} ({:.1}% - {}/{})",
            "▶".blue(),
            crate_display,
            coverage_pct,
            crate_coverage.matched_count,
            crate_coverage.bevy_type_count
        );

        if let Some(module) = &crate_coverage.pybevy_module {
            println!("  PyBevy module: {}", module.dimmed());
        }

        // List types
        for type_cov in &crate_coverage.types {
            if missing_only && type_cov.is_implemented {
                continue;
            }

            // Status symbols:
            // ✓ - fully implemented (has methods or no methods expected)
            // ◐ - partial (type exists but 0 methods implemented when Bevy has methods)
            // ✗ - missing (not implemented at all)
            let status = if type_cov.is_implemented {
                if type_cov.bevy_method_count > 0 && type_cov.matched_method_count == 0 {
                    "◐".yellow() // Type exists but no methods
                } else {
                    "✓".green()
                }
            } else {
                "✗".red()
            };

            let method_info = if type_cov.is_implemented && type_cov.bevy_method_count > 0 {
                let signature_mismatches = type_cov
                    .methods
                    .iter()
                    .filter(|m| m.is_implemented && !m.signature_matches)
                    .count();
                let perfect_matches = type_cov.matched_method_count - signature_mismatches;

                if signature_mismatches > 0 {
                    format!(
                        " [{}/{} methods: {} ok, {} sig mismatch]",
                        type_cov.matched_method_count,
                        type_cov.bevy_method_count,
                        perfect_matches,
                        signature_mismatches
                    )
                } else {
                    format!(
                        " [{}/{} methods]",
                        type_cov.matched_method_count, type_cov.bevy_method_count
                    )
                }
            } else {
                String::new()
            };

            let field_info = if type_cov.is_implemented && type_cov.bevy_field_count > 0 {
                let ctor_info = if type_cov.constructor_settable_field_count
                    < type_cov.bevy_field_count
                    && !config
                        .bevy
                        .should_ignore_constructor_warnings(&type_cov.bevy_name)
                {
                    format!(
                        " [{}/{} ctor]",
                        type_cov.constructor_settable_field_count, type_cov.bevy_field_count
                    )
                } else {
                    String::new()
                };
                format!(
                    " [{}/{} fields]{}",
                    type_cov.matched_field_count, type_cov.bevy_field_count, ctor_info
                )
            } else {
                String::new()
            };

            let variant_info = if type_cov.is_implemented && type_cov.bevy_variant_count > 0 {
                let extra_suffix = if type_cov.extra_variant_count > 0 {
                    format!(" +{} extra", type_cov.extra_variant_count)
                } else {
                    String::new()
                };
                format!(
                    " [{}/{} variants{}]",
                    type_cov.matched_variant_count, type_cov.bevy_variant_count, extra_suffix
                )
            } else {
                String::new()
            };

            // Add usage info for unimplemented types if available
            let usage_info = if !type_cov.is_implemented {
                if let Some(usage) = usage_map {
                    if let Some(&count) = usage.get(&type_cov.bevy_name) {
                        if count > 0 {
                            format!(" (used {} times in examples)", count)
                                .cyan()
                                .to_string()
                        } else {
                            " (not used in examples)".dimmed().to_string()
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let info_str = format!("{}{}{}", method_info, field_info, variant_info);
            println!(
                "    {} {}{}{}",
                status,
                type_cov.bevy_name,
                info_str.dimmed(),
                usage_info
            );

            // Show methods if requested
            if show_methods && type_cov.is_implemented {
                // Show methods with signature mismatches first
                let mismatched_methods: Vec<_> = type_cov
                    .methods
                    .iter()
                    .filter(|m| m.is_implemented && !m.signature_matches)
                    .collect();

                if !mismatched_methods.is_empty() {
                    println!("        {} Signature mismatches:", "⚠".yellow());
                    for method in &mismatched_methods {
                        print!("          {} {}", "·".yellow(), method.bevy_name);
                        if !method.differences.is_empty() {
                            let diff_str: Vec<String> =
                                method.differences.iter().map(|d| d.to_string()).collect();
                            println!(" ({})", diff_str.join(", ").dimmed());
                        } else {
                            println!();
                        }
                    }
                }

                let missing_methods: Vec<_> = type_cov.missing_methods();
                if !missing_methods.is_empty() {
                    println!("        {} Missing methods:", "✗".red());
                    for method in &missing_methods {
                        if method.missing_required_types.is_empty() {
                            println!("          {} {}", "·".red(), method.bevy_name.dimmed());
                        } else {
                            println!(
                                "          {} {} {}",
                                "·".red(),
                                method.bevy_name.dimmed(),
                                format!("(needs: {})", method.missing_required_types.join(", "))
                                    .cyan()
                            );
                        }
                    }
                }

                let extra_methods = type_cov.extra_methods();
                if !extra_methods.is_empty() {
                    println!("        {} Extra methods (not in Bevy):", "⚠".yellow());
                    for method in extra_methods {
                        let suffix = if method.is_property {
                            " (property)"
                        } else if method.is_static {
                            " (static)"
                        } else {
                            ""
                        };
                        println!(
                            "          {} {}{}",
                            "·".yellow(),
                            method.pybevy_name.yellow(),
                            suffix.dimmed()
                        );
                    }
                }

                let intentional_extras = config
                    .bevy
                    .get_intentional_extra_methods(&type_cov.bevy_name);
                if !intentional_extras.is_empty() && verbose {
                    println!("        {} Pythonic extras:", "ℹ".cyan());
                    for method in &intentional_extras {
                        let sig: String = method.signature();
                        println!("          {} {}", "·".cyan(), sig.cyan());
                    }
                }

                if verbose
                    && let Some(type_ignores) = config.bevy.get_type_ignores(&type_cov.bevy_name)
                {
                    let has_omissions = !type_ignores.intentional_missing_methods.is_empty()
                        || !type_ignores.intentional_missing_fields.is_empty();
                    if has_omissions {
                        println!("        {} Intentionally omitted:", "⊘".dimmed());
                        for method in &type_ignores.intentional_missing_methods {
                            if let Some(note) = method.note() {
                                // Per-item note - show on same line
                                let truncated = if note.len() > 60 {
                                    format!("{}...", &note[..57])
                                } else {
                                    note.to_string()
                                };
                                println!(
                                    "          {} {} {} - {}",
                                    "·".dimmed(),
                                    method.name().dimmed(),
                                    "(method)".dimmed(),
                                    truncated.dimmed()
                                );
                            } else {
                                println!(
                                    "          {} {} {}",
                                    "·".dimmed(),
                                    method.name().dimmed(),
                                    "(method)".dimmed()
                                );
                            }
                        }
                        for field in &type_ignores.intentional_missing_fields {
                            if let Some(note) = field.note() {
                                let truncated = if note.len() > 60 {
                                    format!("{}...", &note[..57])
                                } else {
                                    note.to_string()
                                };
                                println!(
                                    "          {} {} {} - {}",
                                    "·".dimmed(),
                                    field.name().dimmed(),
                                    "(field)".dimmed(),
                                    truncated.dimmed()
                                );
                            } else {
                                println!(
                                    "          {} {} {}",
                                    "·".dimmed(),
                                    field.name().dimmed(),
                                    "(field)".dimmed()
                                );
                            }
                        }
                        // Show type-level notes only if there are items without per-item notes
                        let has_items_without_notes = type_ignores
                            .intentional_missing_methods
                            .iter()
                            .any(|m| m.note().is_none())
                            || type_ignores
                                .intentional_missing_fields
                                .iter()
                                .any(|f| f.note().is_none());
                        if has_items_without_notes && let Some(ref notes) = type_ignores.notes {
                            // Format notes - show first line, truncate if too long
                            let first_line = notes.lines().next().unwrap_or(notes);
                            let truncated = if first_line.len() > 80 {
                                format!("{}...", &first_line[..77])
                            } else {
                                first_line.to_string()
                            };
                            println!("          {} {}", "→".dimmed(), truncated.dimmed());
                        }
                    }
                }

                let missing_fields: Vec<_> = type_cov.missing_fields();
                if !missing_fields.is_empty() {
                    println!("        {} Missing fields:", "✗".red());
                    for field in &missing_fields {
                        let simplified_type = simplify_type_for_display(&field.bevy_type);
                        println!(
                            "          {} {}: {}",
                            "·".red(),
                            field.bevy_name.dimmed(),
                            simplified_type.dimmed()
                        );
                    }
                }

                if type_cov.enum_representation_matches == Some(false) {
                    println!(
                        "        {} Bevy enum is represented by an ordinary PyBevy class",
                        "✗".red()
                    );
                }

                let missing_variants: Vec<_> = type_cov.missing_variants();
                if !missing_variants.is_empty() {
                    println!("        {} Missing variants:", "✗".red());
                    for variant in &missing_variants {
                        println!(
                            "          {} {} ({})",
                            "·".red(),
                            variant.bevy_name.dimmed(),
                            variant.bevy_kind.dimmed()
                        );
                    }
                }

                let mismatched_variants: Vec<_> = type_cov.mismatched_variants();
                if !mismatched_variants.is_empty() {
                    println!("        {} Variant shape mismatches:", "✗".red());
                    for variant in &mismatched_variants {
                        println!(
                            "          {} {} (Bevy {}, PyBevy {})",
                            "·".red(),
                            variant.bevy_name.dimmed(),
                            variant.bevy_kind.dimmed(),
                            variant.pybevy_kind.as_deref().unwrap_or("?").dimmed()
                        );
                    }
                }

                let extra_variants: Vec<_> = type_cov.extra_variants();
                if !extra_variants.is_empty() {
                    println!("        {} Extra variants (not in Bevy):", "⚠".yellow());
                    for variant in &extra_variants {
                        let kind = variant.pybevy_kind.as_deref().unwrap_or("?");
                        println!(
                            "          {} {} ({})",
                            "·".yellow(),
                            variant.bevy_name.yellow(),
                            kind.dimmed()
                        );
                    }
                }

                if !type_cov.constructor_warnings.is_empty() {
                    println!("        {} Constructor warnings:", "⚠".yellow());
                    for warning in &type_cov.constructor_warnings {
                        println!(
                            "          {} '{}' optional but Bevy field is {} (mandatory)",
                            "·".yellow(),
                            warning.param_name.yellow(),
                            warning.bevy_type.dimmed()
                        );
                    }
                }

                let fields_not_in_ctor: Vec<_> = type_cov.fields_not_in_constructor();
                if !fields_not_in_ctor.is_empty()
                    && !config
                        .bevy
                        .should_ignore_constructor_warnings(&type_cov.bevy_name)
                {
                    println!("        {} Fields not in constructor:", "⚠".yellow());
                    for field in &fields_not_in_ctor {
                        let simplified_type = simplify_type_for_display(&field.bevy_type);
                        println!(
                            "          {} {}: {}",
                            "·".yellow(),
                            field.bevy_name.yellow(),
                            simplified_type.dimmed()
                        );
                    }
                }

                if let Some(ref mm) = type_cov.module_mismatch {
                    println!(
                        "        {} Module mismatch: Bevy has this in {} but PyBevy registers it in pybevy_{}",
                        "⚠".yellow(),
                        mm.bevy_crate.cyan(),
                        mm.actual_module.yellow()
                    );
                    println!(
                        "          Expected module: pybevy_{}",
                        mm.expected_module.green()
                    );
                }

                if !type_cov.extends_warnings.is_empty() {
                    use pybevy_lint::comparison::ExtendsWarningKind;

                    // Separate warnings from info notes
                    let (info_notes, real_warnings): (Vec<_>, Vec<_>) =
                        type_cov.extends_warnings.iter().partition(|w| {
                            matches!(w.kind, ExtendsWarningKind::AlternativeExtends { .. })
                        });

                    // Show real warnings first
                    if !real_warnings.is_empty() {
                        println!("        {} Extends/trait warnings:", "⚠".yellow());
                        for warning in &real_warnings {
                            match &warning.kind {
                                ExtendsWarningKind::MissingExtends {
                                    bevy_trait,
                                    expected_base,
                                } => {
                                    println!(
                                        "          {} Bevy implements {} but PyBevy doesn't extend {}",
                                        "·".yellow(),
                                        bevy_trait.cyan(),
                                        expected_base.yellow()
                                    );
                                }
                                ExtendsWarningKind::UnexpectedExtends {
                                    pybevy_extends,
                                    expected_trait,
                                } => {
                                    println!(
                                        "          {} PyBevy extends {} but Bevy doesn't implement {}",
                                        "·".yellow(),
                                        pybevy_extends.yellow(),
                                        expected_trait.cyan()
                                    );
                                }
                                ExtendsWarningKind::MissingEq => {
                                    println!(
                                        "          {} Bevy derives {} but #[pyclass] is missing {}",
                                        "·".yellow(),
                                        "PartialEq".cyan(),
                                        "eq".yellow()
                                    );
                                }
                                ExtendsWarningKind::AlternativeExtends { .. } => {
                                    // Handled in info_notes
                                }
                            }
                        }
                    }

                    if !info_notes.is_empty() {
                        println!("        {} Note (Python single inheritance):", "ℹ".blue());
                        for note in &info_notes {
                            if let ExtendsWarningKind::AlternativeExtends {
                                bevy_trait,
                                matched_trait,
                                ..
                            } = &note.kind
                            {
                                println!(
                                    "          {} Also implements {} (PyBevy uses {})",
                                    "·".dimmed(),
                                    bevy_trait.dimmed(),
                                    matched_trait.cyan()
                                );
                            }
                        }
                    }
                }
            }
        }

        println!();
    }

    if !wrongly_excluded.is_empty() {
        println!(
            "{}",
            "═══════════════════════════════════════════════════════════".bold()
        );
        println!(
            "{} {} excluded types found in Bevy examples:",
            "⚠ Audit Warning:".yellow().bold(),
            wrongly_excluded.len()
        );
        println!(
            "{}",
            "═══════════════════════════════════════════════════════════".bold()
        );
        println!();
        println!("  {:40} {:>10}", "Type".bold(), "Usage".bold());
        println!("  {:40} {:>10}", "─".repeat(40), "─".repeat(10));

        for (type_name, count) in wrongly_excluded.iter().take(15) {
            println!(
                "  {:40} {:>10}",
                type_name.yellow(),
                count.to_string().yellow()
            );
        }
        if wrongly_excluded.len() > 15 {
            println!("  ... and {} more", wrongly_excluded.len() - 15);
        }
        println!();
        println!(
            "  {} These types are in excluded_types but are used in Bevy examples.",
            "Note:".blue().bold()
        );
        println!("  Consider removing them from .pybevy-lint.toml or run 'audit' for details.");
        println!();
    }
}

fn print_coverage_markdown(
    report: &pybevy_lint::CoverageReport,
    missing_only: bool,
    show_methods: bool,
) {
    println!("# PyBevy API Coverage Report\n");

    println!("## Summary\n");
    println!("| Metric | Covered | Total | Coverage |");
    println!("|--------|---------|-------|----------|");
    println!(
        "| Types | {} | {} | {:.1}% |",
        report.matched_types,
        report.total_bevy_types,
        report.type_coverage_percent()
    );
    println!(
        "| Methods (overall) | {} | {} | {:.1}% |",
        report.matched_methods,
        report.total_bevy_methods,
        report.method_coverage_percent()
    );
    println!(
        "| Methods (implemented types) | {} | {} | {:.1}% |",
        report.implemented_type_matched_methods,
        report.implemented_type_bevy_methods,
        report.implemented_method_coverage_percent()
    );
    if report.total_bevy_fields > 0 {
        println!(
            "| Fields (implemented types) | {} | {} | {:.1}% |",
            report.matched_fields,
            report.total_bevy_fields,
            report.field_coverage_percent()
        );
    }
    println!();

    println!("## Per-Crate Coverage\n");

    let mut crates: Vec<_> = report.crates.iter().collect();
    crates.sort_by(|a, b| a.0.cmp(b.0));

    for (crate_name, crate_coverage) in crates {
        let coverage_pct = crate_coverage.coverage_percent();

        println!("### {} ({:.1}%)\n", crate_name, coverage_pct);

        if let Some(module) = &crate_coverage.pybevy_module {
            println!("PyBevy module: `{module}`\n");
        }

        println!("| Type | Status | Methods | Fields |");
        println!("|------|--------|---------|--------|");

        for type_cov in &crate_coverage.types {
            if missing_only && type_cov.is_implemented {
                continue;
            }

            let status = if type_cov.is_implemented {
                if type_cov.bevy_method_count > 0 && type_cov.matched_method_count == 0 {
                    "🔶 Partial" // Type exists but no methods
                } else {
                    "✅ Implemented"
                }
            } else {
                "❌ Missing"
            };

            let method_info = if type_cov.is_implemented && type_cov.bevy_method_count > 0 {
                format!(
                    "{}/{}",
                    type_cov.matched_method_count, type_cov.bevy_method_count
                )
            } else {
                "-".to_string()
            };

            let field_info = if type_cov.is_implemented && type_cov.bevy_field_count > 0 {
                format!(
                    "{}/{}",
                    type_cov.matched_field_count, type_cov.bevy_field_count
                )
            } else {
                "-".to_string()
            };

            println!(
                "| {} | {} | {} | {} |",
                type_cov.bevy_name, status, method_info, field_info
            );
        }

        println!();

        // Show missing methods and fields if requested
        if show_methods {
            let types_with_missing_methods: Vec<_> = crate_coverage
                .types
                .iter()
                .filter(|t| t.is_implemented && !t.missing_methods().is_empty())
                .collect();

            if !types_with_missing_methods.is_empty() {
                println!("#### Missing Methods\n");
                for type_cov in types_with_missing_methods {
                    println!("**{}**:", type_cov.bevy_name);
                    for method in type_cov.missing_methods() {
                        println!("- `{}`", method.bevy_name);
                    }
                    println!();
                }
            }

            let types_with_missing_fields: Vec<_> = crate_coverage
                .types
                .iter()
                .filter(|t| t.is_implemented && !t.missing_fields().is_empty())
                .collect();

            if !types_with_missing_fields.is_empty() {
                println!("#### Missing Fields\n");
                for type_cov in types_with_missing_fields {
                    println!("**{}**:", type_cov.bevy_name);
                    for field in type_cov.missing_fields() {
                        let simplified_type = simplify_type_for_display(&field.bevy_type);
                        println!("- `{}`: `{}`", field.bevy_name, simplified_type);
                    }
                    println!();
                }
            }
        }
    }
}

fn print_coverage_json(report: &pybevy_lint::CoverageReport) {
    println!("{{");
    println!("  \"summary\": {{");
    println!("    \"total_bevy_types\": {},", report.total_bevy_types);
    println!("    \"matched_types\": {},", report.matched_types);
    println!(
        "    \"type_coverage_percent\": {:.1},",
        report.type_coverage_percent()
    );
    println!("    \"total_bevy_methods\": {},", report.total_bevy_methods);
    println!("    \"matched_methods\": {},", report.matched_methods);
    println!(
        "    \"method_coverage_percent\": {:.1},",
        report.method_coverage_percent()
    );
    println!("    \"total_bevy_fields\": {},", report.total_bevy_fields);
    println!("    \"matched_fields\": {},", report.matched_fields);
    println!(
        "    \"field_coverage_percent\": {:.1},",
        report.field_coverage_percent()
    );
    println!(
        "    \"total_bevy_variants\": {},",
        report.total_bevy_variants
    );
    println!("    \"matched_variants\": {},", report.matched_variants);
    println!("    \"missing_variants\": {},", report.missing_variants);
    println!(
        "    \"mismatched_variants\": {},",
        report.mismatched_variants
    );
    println!("    \"extra_variants\": {},", report.extra_variants);
    println!(
        "    \"enum_representation_mismatches\": {}",
        report.enum_representation_mismatches
    );
    println!("  }},");
    println!("  \"crates\": {{");

    let crates: Vec<_> = report.crates.iter().collect();
    for (i, (crate_name, crate_coverage)) in crates.iter().enumerate() {
        println!("    \"{}\": {{", crate_name);
        println!(
            "      \"bevy_type_count\": {},",
            crate_coverage.bevy_type_count
        );
        println!("      \"matched_count\": {},", crate_coverage.matched_count);
        println!(
            "      \"coverage_percent\": {:.1}",
            crate_coverage.coverage_percent()
        );
        if i < crates.len() - 1 {
            println!("    }},");
        } else {
            println!("    }}");
        }
    }

    println!("  }}");
    println!("}}");
}

fn format_class_summary(class: &pybevy_lint::model::PyClassDef) -> String {
    use std::fmt::Write;
    let mut s = String::new();

    writeln!(
        s,
        "{} {} (Rust: {})",
        "class".cyan(),
        class.python_name.bold(),
        class.rust_name
    )
    .unwrap();

    if let Some(ref extends) = class.extends {
        writeln!(s, "  extends: {}", extends).unwrap();
    }

    if let Some(ref loc) = class.location {
        writeln!(s, "  location: {}:{}", loc.file.display(), loc.line).unwrap();
    }

    if let Some(ref macro_info) = class.macro_info {
        writeln!(s, "  macro: {:?}", macro_info).unwrap();
    }

    if let Some(ref ctor) = class.constructor {
        write!(s, "  constructor: __init__(").unwrap();
        for (i, param) in ctor.parameters.iter().enumerate() {
            if i > 0 {
                write!(s, ", ").unwrap();
            }
            write!(s, "{}", param.name).unwrap();
            if let Some(ref ty) = param.param_type {
                write!(s, ": {}", ty).unwrap();
            }
            if let Some(ref default) = param.default_value {
                write!(s, " = {}", default).unwrap();
            }
        }
        writeln!(s, ")").unwrap();
    }

    if !class.properties.is_empty() {
        writeln!(s, "  properties:").unwrap();
        for prop in &class.properties {
            let access = match (prop.has_getter, prop.has_setter) {
                (true, true) => "rw",
                (true, false) => "ro",
                (false, true) => "wo",
                (false, false) => "??",
            };
            writeln!(
                s,
                "    {} [{}]: {}",
                prop.name,
                access,
                prop.property_type.as_deref().unwrap_or("?")
            )
            .unwrap();
        }
    }

    if !class.methods.is_empty() {
        writeln!(s, "  methods:").unwrap();
        for method in &class.methods {
            write!(s, "    {}(", method.name).unwrap();
            for (i, param) in method.parameters.iter().enumerate() {
                if i > 0 {
                    write!(s, ", ").unwrap();
                }
                write!(s, "{}", param.name).unwrap();
            }
            write!(s, ")").unwrap();
            if let Some(ref ret) = method.return_type {
                write!(s, " -> {}", ret).unwrap();
            }
            writeln!(s).unwrap();
        }
    }

    if !class.static_methods.is_empty() {
        writeln!(s, "  static methods:").unwrap();
        for method in &class.static_methods {
            writeln!(s, "    {}()", method.name).unwrap();
        }
    }

    if !class.class_attrs.is_empty() {
        writeln!(s, "  class attributes:").unwrap();
        for attr in &class.class_attrs {
            writeln!(s, "    {}", attr.name).unwrap();
        }
    }

    s
}
